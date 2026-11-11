//! Журнал владения: сначала намерение на диске, затем изменение, затем подтверждение.
//! Значения и адреса никогда не включаются в сообщения об ошибках или отчёты.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resource {
    WindowsProxy(String),
    MacProxy { service: String, key: String },
    Json { path: PathBuf, keys: Vec<String> },
    Shell(PathBuf),
}

impl Resource {
    pub fn is_system(&self) -> bool {
        matches!(self, Self::WindowsProxy(_) | Self::MacProxy { .. })
    }
}

/// None означает отсутствие, Some(null), пустая строка и нулевой DWORD — отдельные состояния.
pub trait Backend {
    fn read(&mut self, resource: &Resource) -> Result<Option<Value>, String>;
    fn write(&mut self, resource: &Resource, value: &Option<Value>) -> Result<(), String>;
    fn is_original(
        &self,
        resource: &Resource,
        current: &Option<Value>,
        original: &Option<Value>,
    ) -> bool {
        let contextual = matches!(resource, Resource::WindowsProxy(name) if name == "ProxyEnable")
            || matches!(resource, Resource::MacProxy { key, .. } if matches!(key.as_str(), "HTTPEnable" | "HTTPSEnable" | "ProxyAutoConfigEnable"));
        if contextual {
            if let (Some(current), Some(original)) = (current, original) {
                if let (Some(current), Some(original)) = (current.get("raw"), original.get("raw")) {
                    return current == original;
                }
            }
        }
        current == original
    }
    fn finish_restore(&mut self, _resource: &Resource) -> Result<(), String> {
        Ok(())
    }
    fn intermediates(
        &self,
        _resource: &Resource,
        _from: &Option<Value>,
        _to: &Option<Value>,
    ) -> Vec<Option<Value>> {
        vec![]
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Pending {
    value: Option<Value>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    resource: Resource,
    original: Option<Value>,
    expected: Option<Value>,
    // При обновлении собственного значения нужно пережить аварию до и после записи ОС.
    pending: Option<Pending>,
    #[serde(default)]
    intermediates: Vec<Option<Value>>,
}

#[derive(Serialize, Deserialize)]
struct State {
    version: u32,
    entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Outcome {
    Restored,
    NotOwned,
    Conflict,
    Error,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryReport {
    pub outcome: Outcome,
    // Только порядковый номер: пути, URL и секреты не попадают в отчёт.
    pub item: usize,
}

pub struct Journal {
    path: PathBuf,
    state: State,
    _lock: File,
}

fn error() -> String {
    "Не удалось безопасно прочитать или сохранить журнал владения".into()
}

/// Атомарная замена в том же каталоге. До rename данные синхронизированы на диск.
pub(super) fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or_else(error)?;
    fs::create_dir_all(parent).map_err(|_| error())?;
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Символическая ссылка: автоматическая правка отменена".into());
    }
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|_| error())?;
    if let Ok(metadata) = fs::metadata(path) {
        tmp.as_file()
            .set_permissions(metadata.permissions())
            .map_err(|_| error())?;
    }
    tmp.write_all(data).map_err(|_| error())?;
    tmp.as_file().sync_all().map_err(|_| error())?;
    tmp.persist(path).map_err(|_| error())?;
    #[cfg(unix)]
    File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| error())?;
    Ok(())
}

fn private_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|_| error())?;
    if fs::symlink_metadata(path)
        .map_err(|_| error())?
        .file_type()
        .is_symlink()
    {
        return Err(error());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|_| error())?;
    }
    #[cfg(all(target_os = "macos", not(test)))]
    {
        let output = super::silent_command("/usr/bin/tmutil")
            .arg("addexclusion")
            .arg(path)
            .output()
            .map_err(|_| error())?;
        if !output.status.success() {
            return Err("Не удалось исключить журнал с секретами из резервного копирования".into());
        }
    }
    #[cfg(all(windows, not(test)))]
    {
        // LOCALAPPDATA обычно уже закрыт ACL пользователя, но журнал явно ограничивается SID.
        let output = super::silent_command("whoami")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()
            .map_err(|_| error())?;
        if !output.status.success() {
            return Err(error());
        }
        let text = String::from_utf8(output.stdout).map_err(|_| error())?;
        let sid = text
            .split(',')
            .next_back()
            .ok_or_else(error)?
            .trim()
            .trim_matches('"');
        if !sid.starts_with("S-1-")
            || !sid
                .chars()
                .all(|c| c.is_ascii_digit() || c == '-' || c == 'S')
        {
            return Err(error());
        }
        let status = super::silent_command("icacls")
            .arg(path)
            .args(["/inheritance:r", "/grant:r", &format!("*{sid}:(OI)(CI)F")])
            .output()
            .map_err(|_| error())?;
        if !status.status.success() {
            return Err(error());
        }
    }
    Ok(())
}

impl Journal {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let parent = path.parent().ok_or_else(error)?;
        private_directory(parent)?;
        let lock_path = parent.join("ownership.lock");
        if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(error());
        }
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(|_| error())?;
        lock.try_lock()
            .map_err(|_| "Журнал занят другим процессом Zastava".to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            lock.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| error())?;
            if path.exists() {
                if fs::symlink_metadata(&path)
                    .map_err(|_| error())?
                    .file_type()
                    .is_symlink()
                {
                    return Err(error());
                }
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                    .map_err(|_| error())?;
            }
        }
        let state = match fs::read(&path) {
            Ok(data) => serde_json::from_slice::<State>(&data).map_err(|_| error())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State {
                version: 1,
                entries: vec![],
            },
            Err(_) => return Err(error()),
        };
        if state.version != 1 {
            return Err("Неизвестная версия журнала: настройки сохранены без изменений".into());
        }
        Ok(Self {
            path,
            state,
            _lock: lock,
        })
    }

    fn save(&self) -> Result<(), String> {
        atomic_write(
            &self.path,
            &serde_json::to_vec(&self.state).map_err(|_| error())?,
        )
    }

    pub fn apply(
        &mut self,
        backend: &mut impl Backend,
        resource: Resource,
        desired: Option<Value>,
    ) -> Result<bool, String> {
        let current = backend.read(&resource)?;
        let index = self
            .state
            .entries
            .iter()
            .position(|e| e.resource == resource);
        if let Some(i) = index {
            let e = &self.state.entries[i];
            if current != e.expected
                && e.pending.as_ref().is_none_or(|p| p.value != current)
                && !e.intermediates.contains(&current)
            {
                return Err("Конфликт владения: внешнее изменение сохранено".into());
            }
        }
        if current == desired {
            if let Some(i) = index {
                if self.state.entries[i].pending.is_some() {
                    self.state.entries[i].expected = current;
                    self.state.entries[i].pending = None;
                    self.save()?;
                }
            }
            return Ok(false);
        }
        let i = match index {
            Some(i) => i,
            None => {
                self.state.entries.push(Entry {
                    resource: resource.clone(),
                    original: current.clone(),
                    expected: current.clone(),
                    pending: None,
                    intermediates: vec![],
                });
                self.state.entries.len() - 1
            }
        };
        self.state.entries[i].expected = current.clone();
        self.state.entries[i].pending = Some(Pending {
            value: desired.clone(),
        });
        self.state.entries[i].intermediates = backend.intermediates(&resource, &current, &desired);
        self.save()?;
        // Повторное чтение сокращает окно гонки с VPN/редактором; внешние API не имеют CAS.
        if backend.read(&resource)? != current {
            return Err("Конфликт перед записью: параметр не изменён".into());
        }
        backend.write(&resource, &desired)?;
        if backend.read(&resource)? != desired {
            return Err("Система не подтвердила изменение параметра".into());
        }
        self.state.entries[i].expected = desired;
        self.state.entries[i].pending = None;
        self.state.entries[i].intermediates.clear();
        self.save()?;
        Ok(true)
    }

    /// До собственной смены PAC URL фиксируем оба допустимых контекста его флага.
    /// Сам флаг здесь не записывается; исходное значение остаётся прежним.
    pub fn prepare_context(
        &mut self,
        backend: &mut impl Backend,
        resource: &Resource,
        after: Option<Value>,
    ) -> Result<(), String> {
        let Some(i) = self
            .state
            .entries
            .iter()
            .position(|e| &e.resource == resource)
        else {
            return Ok(());
        };
        self.check(backend, resource)?;
        let current = backend.read(resource)?;
        if !backend.is_original(resource, &current, &after) {
            return Err("Обновление контекста не должно менять само значение".into());
        }
        self.state.entries[i].expected = current;
        self.state.entries[i].pending = Some(Pending { value: after });
        self.save()
    }

    pub fn check(&self, backend: &mut impl Backend, resource: &Resource) -> Result<(), String> {
        if let Some(e) = self.state.entries.iter().find(|e| &e.resource == resource) {
            let current = backend.read(resource)?;
            if current != e.expected
                && e.pending.as_ref().is_none_or(|p| p.value != current)
                && !e.intermediates.contains(&current)
            {
                return Err("Конфликт владения: внешнее изменение сохранено".into());
            }
        }
        Ok(())
    }

    pub fn original(&self, resource: &Resource) -> Option<Option<Value>> {
        self.state
            .entries
            .iter()
            .find(|e| &e.resource == resource)
            .map(|e| e.original.clone())
    }

    pub fn resources(&self, system: bool) -> Vec<Resource> {
        self.state
            .entries
            .iter()
            .filter(|e| e.resource.is_system() == system)
            .map(|e| e.resource.clone())
            .collect()
    }

    /// Ошибка одного ресурса не мешает остальным. Неуспешные записи остаются для повторения.
    pub fn restore(&mut self, backend: &mut impl Backend, system: bool) -> Vec<RecoveryReport> {
        let resources = self.resources(system);
        resources
            .into_iter()
            .rev()
            .enumerate()
            .map(|(item, resource)| {
                let outcome = self
                    .restore_one(backend, &resource)
                    .unwrap_or(Outcome::Error);
                RecoveryReport { item, outcome }
            })
            .collect()
    }

    pub fn restore_one(
        &mut self,
        backend: &mut impl Backend,
        resource: &Resource,
    ) -> Result<Outcome, String> {
        let Some(i) = self
            .state
            .entries
            .iter()
            .position(|e| &e.resource == resource)
        else {
            return Ok(Outcome::NotOwned);
        };
        let e = self.state.entries[i].clone();
        let current = backend.read(resource)?;
        if !backend.is_original(resource, &current, &e.original) {
            if current != e.expected
                && e.pending.as_ref().is_none_or(|p| p.value != current)
                && !e.intermediates.contains(&current)
            {
                return Ok(Outcome::Conflict);
            }
            if backend.read(resource)? != current {
                return Ok(Outcome::Conflict);
            }
            self.state.entries[i]
                .intermediates
                .extend(backend.intermediates(resource, &current, &e.original));
            self.save()?;
            backend.write(resource, &e.original)?;
            let restored = backend.read(resource)?;
            if !backend.is_original(resource, &restored, &e.original) {
                return Err("Восстановление не подтверждено системой".into());
            }
        }
        backend.finish_restore(resource)?;
        self.state.entries.remove(i);
        self.save()?;
        Ok(Outcome::Restored)
    }
}

#[cfg(not(test))]
pub(super) fn default_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = super::user_home_dir().map(|p| p.join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = super::user_home_dir().map(|p| p.join(".cache"));
    Ok(base
        .ok_or_else(error)?
        .join("Zastava/owned-settings-v1/state.json"))
}

#[cfg(test)]
pub(super) fn default_path() -> Result<PathBuf, String> {
    // Старые файловые unit tests тоже никогда не пишут журнал в настоящий HOME.
    thread_local! { static DIR: tempfile::TempDir = tempfile::tempdir().expect("Временный каталог теста"); }
    Ok(DIR.with(|p| p.path().join("journal/state.json")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[derive(Default)]
    struct Fake {
        values: Vec<(Resource, Option<Value>)>,
        fail: Option<Resource>,
        crash_after_write: bool,
        fail_before_write: bool,
        writes: usize,
    }
    impl Fake {
        fn put(&mut self, r: Resource, v: Option<Value>) {
            self.values.retain(|(key, _)| key != &r);
            self.values.push((r, v));
        }
    }
    impl Backend for Fake {
        fn read(&mut self, r: &Resource) -> Result<Option<Value>, String> {
            if self.fail.as_ref() == Some(r) {
                return Err("Смоделированный отказ чтения".into());
            }
            Ok(self
                .values
                .iter()
                .find(|(key, _)| key == r)
                .and_then(|(_, v)| v.clone()))
        }
        fn write(&mut self, r: &Resource, v: &Option<Value>) -> Result<(), String> {
            if self.fail_before_write {
                return Err("Смоделирован отказ до записи ОС".into());
            }
            self.writes += 1;
            self.put(r.clone(), v.clone());
            if self.crash_after_write {
                return Err("Смоделирована авария после записи ОС".into());
            }
            Ok(())
        }
    }
    fn resource(name: &str) -> Resource {
        Resource::WindowsProxy(name.into())
    }

    #[test]
    fn t01_t03_exact_types_absence_empty_and_proxy_protocol_map() {
        for original in [
            None,
            Some(Value::Null),
            Some(json!("")),
            Some(json!({"kind":2,"bytes":[37,0,80,0,37,0]})),
            Some(json!("http=a:1;https=b:2")),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut j = Journal::open(dir.path().join("state.json")).unwrap();
            let r = resource("AutoConfigURL");
            let mut os = Fake::default();
            os.put(r.clone(), original.clone());
            j.apply(&mut os, r.clone(), Some(json!("zastava.pac")))
                .unwrap();
            assert_eq!(j.restore(&mut os, true)[0].outcome, Outcome::Restored);
            assert_eq!(os.read(&r).unwrap(), original);
            assert!(j.restore(&mut os, true).is_empty());
        }
    }

    #[test]
    fn t02_t05_t08_repeated_enable_new_pac_and_external_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let r = resource("AutoConfigURL");
        let mut os = Fake::default();
        os.put(r.clone(), Some(json!("foreign.pac")));
        for _ in 0..3 {
            let mut j = Journal::open(path.clone()).unwrap();
            j.apply(&mut os, r.clone(), Some(json!("own1.pac")))
                .unwrap();
            j.apply(&mut os, r.clone(), Some(json!("own2.pac")))
                .unwrap();
            j.apply(&mut os, r.clone(), Some(json!("own2.pac")))
                .unwrap();
            assert_eq!(j.restore(&mut os, true)[0].outcome, Outcome::Restored);
            assert_eq!(os.read(&r).unwrap(), Some(json!("foreign.pac")));
        }
        let mut j = Journal::open(path).unwrap();
        j.apply(&mut os, r.clone(), Some(json!("own.pac"))).unwrap();
        os.put(r.clone(), Some(json!("throne.pac")));
        assert!(j.apply(&mut os, r.clone(), Some(json!("own.pac"))).is_err());
        assert_eq!(j.restore(&mut os, true)[0].outcome, Outcome::Conflict);
        assert_eq!(os.read(&r).unwrap(), Some(json!("throne.pac")));
    }

    #[test]
    fn t06_crash_before_after_apply_and_during_owned_update() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let r = resource("AutoConfigURL");
        let mut os = Fake::default();
        {
            let mut j = Journal::open(path.clone()).unwrap();
            // Авария сразу после durable intent, до записи ОС.
            j.state.entries.push(Entry {
                resource: r.clone(),
                original: None,
                expected: None,
                pending: Some(Pending {
                    value: Some(json!("own")),
                }),
                intermediates: vec![],
            });
            j.save().unwrap();
        }
        let mut j = Journal::open(path.clone()).unwrap();
        assert_eq!(j.restore(&mut os, true)[0].outcome, Outcome::Restored);
        j.apply(&mut os, r.clone(), Some(json!("own1"))).unwrap();
        os.crash_after_write = true;
        assert!(j.apply(&mut os, r.clone(), Some(json!("own2"))).is_err());
        drop(j);
        os.crash_after_write = false;
        let mut j = Journal::open(path).unwrap();
        assert_eq!(j.restore(&mut os, true)[0].outcome, Outcome::Restored);
        assert_eq!(os.read(&r).unwrap(), None);
    }

    #[test]
    fn unfinished_update_keeps_previous_applied_value_for_next_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let r = resource("AutoConfigURL");
        let mut os = Fake::default();
        let mut j = Journal::open(path.clone()).unwrap();
        os.crash_after_write = true;
        assert!(j.apply(&mut os, r.clone(), Some(json!("own1"))).is_err());
        os.crash_after_write = false;
        os.fail_before_write = true;
        assert!(j.apply(&mut os, r.clone(), Some(json!("own2"))).is_err());
        drop(j);
        os.fail_before_write = false;
        let mut j = Journal::open(path).unwrap();
        assert_eq!(j.restore(&mut os, true)[0].outcome, Outcome::Restored);
        assert_eq!(os.read(&r).unwrap(), None);
    }

    #[test]
    fn durable_intent_failure_prevents_os_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut j = Journal::open(path.clone()).unwrap();
        fs::create_dir(path).unwrap();
        let mut os = Fake::default();
        assert!(
            j.apply(&mut os, resource("AutoConfigURL"), Some(json!("own")))
                .is_err()
        );
        assert_eq!(os.writes, 0);
    }

    #[test]
    fn t06_pending_absence_survives_serialization() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let r = resource("AutoConfigURL");
        let mut os = Fake::default();
        os.put(r.clone(), Some(json!("old")));
        let mut j = Journal::open(path.clone()).unwrap();
        os.crash_after_write = true;
        assert!(j.apply(&mut os, r.clone(), None).is_err());
        drop(j);
        os.crash_after_write = false;
        let mut j = Journal::open(path).unwrap();
        assert_eq!(j.restore(&mut os, true)[0].outcome, Outcome::Restored);
        assert_eq!(os.read(&r).unwrap(), Some(json!("old")));
    }

    #[test]
    fn t09_t11_macos_services_are_independent_and_conflicts_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = Journal::open(dir.path().join("state.json")).unwrap();
        let mut os = Fake::default();
        let wifi = Resource::MacProxy {
            service: "wifi-id".into(),
            key: "ProxyAutoConfigURLString".into(),
        };
        let eth = Resource::MacProxy {
            service: "ethernet-id".into(),
            key: "ProxyAutoConfigURLString".into(),
        };
        os.put(wifi.clone(), Some(json!("user.pac")));
        j.apply(&mut os, wifi.clone(), Some(json!("own.pac")))
            .unwrap();
        j.apply(&mut os, eth.clone(), Some(json!("own.pac")))
            .unwrap();
        os.put(wifi.clone(), Some(json!("vpn.pac")));
        let result = j.restore(&mut os, true);
        assert!(result.iter().any(|r| r.outcome == Outcome::Conflict));
        assert_eq!(os.read(&wifi).unwrap(), Some(json!("vpn.pac")));
        assert_eq!(os.read(&eth).unwrap(), None);
    }

    #[test]
    fn t10_distinct_http_https_context_is_never_flattened() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = Journal::open(dir.path().join("state.json")).unwrap();
        let mut os = Fake::default();
        for (key, host) in [("HTTPEnable", "first:80"), ("HTTPSEnable", "second:443")] {
            let r = Resource::MacProxy {
                service: "wifi".into(),
                key: key.into(),
            };
            os.put(
                r.clone(),
                Some(json!({"enabled":true,"host":host,"auth":true})),
            );
            j.apply(
                &mut os,
                r,
                Some(json!({"enabled":false,"host":host,"auth":true})),
            )
            .unwrap();
        }
        assert!(
            j.restore(&mut os, true)
                .iter()
                .all(|r| r.outcome == Outcome::Restored)
        );
        assert_eq!(os.values[0].1.as_ref().unwrap()["host"], "second:443");
        assert_eq!(os.values[1].1.as_ref().unwrap()["host"], "first:80");
    }

    #[test]
    fn t18_t19_failure_does_not_block_other_resources_and_retry_is_safe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut j = Journal::open(path.clone()).unwrap();
        let mut os = Fake::default();
        let a = resource("A");
        let b = resource("B");
        j.apply(&mut os, a.clone(), Some(json!(1))).unwrap();
        j.apply(&mut os, b.clone(), Some(json!(2))).unwrap();
        os.fail = Some(a.clone());
        let result = j.restore(&mut os, true);
        assert!(result.iter().any(|r| r.outcome == Outcome::Error));
        assert_eq!(os.read(&b).unwrap(), None);
        drop(j);
        os.fail = None;
        let mut j = Journal::open(path).unwrap();
        assert_eq!(j.restore(&mut os, true)[0].outcome, Outcome::Restored);
        assert!(j.restore(&mut os, true).is_empty());
    }

    #[test]
    fn t20_corrupt_unknown_schema_and_concurrent_process_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let j = Journal::open(path.clone()).unwrap();
        assert!(Journal::open(path.clone()).is_err());
        drop(j);
        fs::write(&path, b"corrupt").unwrap();
        assert!(Journal::open(path.clone()).is_err());
        fs::write(&path, br#"{"version":999,"entries":[]}"#).unwrap();
        assert!(Journal::open(path).is_err());
    }

    #[test]
    fn t20_no_secrets_in_reports_and_private_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("private/state.json");
        let mut j = Journal::open(path.clone()).unwrap();
        let mut os = Fake::default();
        let r = resource("AutoConfigURL");
        os.put(r.clone(), Some(json!("http://user:secret@host")));
        j.apply(&mut os, r.clone(), Some(json!("own"))).unwrap();
        os.put(r, Some(json!("other")));
        let reports = serde_json::to_string(&j.restore(&mut os, true)).unwrap();
        assert!(!reports.contains("secret"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
    }

    #[test]
    fn t21_t22_unknown_localhost_and_external_values_are_not_owned() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = Journal::open(dir.path().join("state.json")).unwrap();
        let mut os = Fake::default();
        os.put(
            resource("AutoConfigURL"),
            Some(json!("http://127.0.0.1:8899/proxy.pac")),
        );
        assert!(j.restore(&mut os, true).is_empty());
        assert_eq!(os.writes, 0);
    }

    #[test]
    fn changed_pac_url_does_not_disable_foreign_pac_flag() {
        for resource in [
            resource("ProxyEnable"),
            Resource::MacProxy {
                service: "wifi".into(),
                key: "ProxyAutoConfigEnable".into(),
            },
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut j = Journal::open(dir.path().join("state.json")).unwrap();
            let mut os = Fake::default();
            os.put(resource.clone(), Some(json!({"raw":0,"url":"user"})));
            j.apply(
                &mut os,
                resource.clone(),
                Some(json!({"raw":1,"url":"own"})),
            )
            .unwrap();
            os.put(resource.clone(), Some(json!({"raw":1,"url":"vpn"})));
            assert_eq!(j.restore(&mut os, true)[0].outcome, Outcome::Conflict);
            assert_eq!(
                os.read(&resource).unwrap(),
                Some(json!({"raw":1,"url":"vpn"}))
            );
        }
    }

    #[test]
    fn own_pac_url_update_keeps_original_flag_and_survives_crash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let r = resource("ProxyEnable");
        let mut j = Journal::open(path.clone()).unwrap();
        let mut os = Fake::default();
        os.put(r.clone(), Some(json!({"raw":1,"pac":"user"})));
        j.apply(&mut os, r.clone(), Some(json!({"raw":0,"pac":"own1"})))
            .unwrap();
        j.prepare_context(&mut os, &r, Some(json!({"raw":0,"pac":"own2"})))
            .unwrap();
        os.put(r.clone(), Some(json!({"raw":0,"pac":"own2"})));
        drop(j);
        let mut j = Journal::open(path).unwrap();
        assert!(
            !j.apply(&mut os, r.clone(), Some(json!({"raw":0,"pac":"own2"})))
                .unwrap()
        );
        assert_eq!(j.restore(&mut os, true)[0].outcome, Outcome::Restored);
        assert_eq!(os.read(&r).unwrap().unwrap()["raw"], 1);
    }

    #[test]
    fn system_cleanup_does_not_touch_client_tz() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = Journal::open(dir.path().join("state.json")).unwrap();
        let mut os = Fake::default();
        let tz = Resource::Json {
            path: dir.path().join("settings.json"),
            keys: vec!["TZ".into()],
        };
        j.apply(&mut os, tz.clone(), Some(json!("Europe/Paris")))
            .unwrap();
        assert!(j.restore(&mut os, true).is_empty());
        assert_eq!(os.read(&tz).unwrap(), Some(json!("Europe/Paris")));
    }
}
