//! Точечные правки JSONC и маркированных профилей без сериализации всего документа.
use super::owned_settings::{Backend, Journal, Outcome, Resource, atomic_write, default_path};
use super::{ZASTAVA_RC_END, ZASTAVA_RC_START, sanitize_jsonc};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub(super) struct Files;

#[derive(Clone)]
struct Member {
    key: String,
    start: usize,
    value: std::ops::Range<usize>,
    comma: Option<usize>,
}

fn skip(text: &str, mut i: usize) -> Result<usize, String> {
    let b = text.as_bytes();
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b.get(i..i + 2) == Some(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b.get(i..i + 2) == Some(b"/*") {
            i += 2;
            while i + 1 < b.len() && &b[i..i + 2] != b"*/" {
                i += 1;
            }
            if i + 1 >= b.len() {
                return Err("Незавершённый комментарий JSONC".into());
            }
            i += 2;
        } else {
            return Ok(i);
        }
    }
}

fn value_end(text: &str, start: usize) -> Result<usize, String> {
    let b = text.as_bytes();
    let mut i = start;
    let mut depth = 0usize;
    let mut string = false;
    while i < b.len() {
        let c = b[i];
        if string {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == b'"' {
                string = false;
                if depth == 0 {
                    return Ok(i + 1);
                }
            }
        } else {
            if c == b'"' {
                string = true;
            } else if c == b'{' || c == b'[' {
                depth += 1;
            } else if c == b'}' || c == b']' {
                if depth == 0 {
                    return Ok(i);
                }
                depth -= 1;
                if depth == 0 {
                    return Ok(i + 1);
                }
            } else if depth == 0 && (c == b',' || c.is_ascii_whitespace()) {
                return Ok(i);
            } else if c == b'/' && matches!(b.get(i + 1), Some(b'/') | Some(b'*')) {
                if depth == 0 {
                    return Ok(i);
                }
                i = skip(text, i)?;
                continue;
            }
        }
        i += 1;
    }
    if string || depth > 0 {
        return Err("Незавершённое значение JSONC".into());
    }
    Ok(i)
}

fn members(text: &str, start: usize) -> Result<(Vec<Member>, usize), String> {
    if text.as_bytes().get(start) != Some(&b'{') {
        return Err("Родитель ключа JSONC не является объектом".into());
    }
    let mut list = Vec::<Member>::new();
    let mut i = skip(text, start + 1)?;
    while text.as_bytes().get(i) != Some(&b'}') {
        let key_start = i;
        if text.as_bytes().get(i) != Some(&b'"') {
            return Err("Ожидался ключ JSONC".into());
        }
        let end = value_end(text, i)?;
        let key: String = serde_json::from_str(&text[i..end]).map_err(|_| "Неверный ключ JSONC")?;
        if list.iter().any(|m| m.key == key) {
            return Err("Повторяющийся ключ JSONC: правка отменена".into());
        }
        i = skip(text, end)?;
        if text.as_bytes().get(i) != Some(&b':') {
            return Err("Ожидалось двоеточие JSONC".into());
        }
        let start_value = skip(text, i + 1)?;
        let end_value = value_end(text, start_value)?;
        i = skip(text, end_value)?;
        let comma = (text.as_bytes().get(i) == Some(&b',')).then_some(i);
        list.push(Member {
            key,
            start: key_start,
            value: start_value..end_value,
            comma,
        });
        if comma.is_some() {
            i = skip(text, i + 1)?;
        } else if text.as_bytes().get(i) != Some(&b'}') {
            return Err("Неверный разделитель JSONC".into());
        }
    }
    Ok((list, i))
}

fn checked(text: &str) -> Result<usize, String> {
    serde_json::from_str::<Value>(&sanitize_jsonc(text.trim_start_matches('\u{feff}')))
        .map_err(|_| "Повреждённый JSONC: файл оставлен без изменений")?;
    skip(text, if text.starts_with('\u{feff}') { 3 } else { 0 })
}

fn locate(text: &str, keys: &[String]) -> Result<(Vec<Member>, usize), String> {
    let mut start = checked(text)?;
    for key in &keys[..keys.len() - 1] {
        let (list, _) = members(text, start)?;
        let Some(m) = list.iter().find(|m| &m.key == key) else {
            return Err("Отсутствует родитель ключа JSONC".into());
        };
        start = m.value.start;
    }
    members(text, start)
}

pub(super) fn read_key(text: &str, keys: &[String]) -> Result<Option<Value>, String> {
    let mut start = checked(text)?;
    for (index, key) in keys.iter().enumerate() {
        let (list, _) = members(text, start)?;
        let Some(m) = list.iter().find(|m| &m.key == key) else {
            return Ok(None);
        };
        if index + 1 == keys.len() {
            let raw = &text[m.value.clone()];
            // Пустые созданные контейнеры сравниваются независимо от отступов.
            if raw
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                == "{}"
            {
                return Ok(Some(json!("{}")));
            }
            return Ok(Some(json!(raw)));
        }
        start = m.value.start;
    }
    Err("Пустой путь JSONC".into())
}

pub(super) fn edit_key(text: &str, keys: &[String], raw: Option<&str>) -> Result<String, String> {
    let (list, close) = locate(text, keys)?;
    let key = keys.last().ok_or("Пустой путь JSONC")?;
    let mut result = text.to_string();
    if let Some((index, m)) = list.iter().enumerate().find(|(_, m)| &m.key == key) {
        if let Some(raw) = raw {
            result.replace_range(m.value.clone(), raw);
        } else if let Some(comma) = m.comma {
            result.replace_range(m.start..comma + 1, "");
        } else {
            result.replace_range(m.start..m.value.end, "");
            if index > 0 {
                if let Some(comma) = list[index - 1].comma {
                    result.replace_range(comma..comma + 1, "");
                }
            }
        }
    } else if let Some(raw) = raw {
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let separator = if list.last().is_some_and(|m| m.comma.is_none()) {
            ","
        } else {
            ""
        };
        result.insert_str(
            close,
            &format!(
                "{separator}{newline}  {}: {raw}{newline}",
                serde_json::to_string(key).map_err(|_| "Неверный ключ")?
            ),
        );
    }
    checked(&result)?;
    Ok(result)
}

fn read_text(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("{}".into()),
        Err(_) => Err("Не удалось прочитать конфигурацию; файл оставлен без изменений".into()),
    }
}

/// Принимаются только отдельные полные строки маркеров; неоднозначные блоки не правятся.
pub(super) fn shell_range(text: &str) -> Result<Option<std::ops::Range<usize>>, String> {
    let mut offset = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let mut start = None;
    let mut range = None;
    for line in text[offset..].split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        if content == ZASTAVA_RC_START {
            if start.is_some() || range.is_some() {
                return Err("Неоднозначные маркеры профиля: правка отменена".into());
            }
            start = Some(offset);
        } else if content == ZASTAVA_RC_END {
            let begin = start.take().ok_or("Конечный маркер без начального")?;
            range = Some(begin..offset + line.len());
        }
        offset += line.len();
    }
    if start.is_some() {
        return Err("Незавершённый блок профиля: правка отменена".into());
    }
    Ok(range)
}

impl Backend for Files {
    fn read(&mut self, resource: &Resource) -> Result<Option<Value>, String> {
        match resource {
            Resource::Json { path, keys } => read_key(&read_text(path)?, keys),
            Resource::Shell(path) => {
                let text = match fs::read_to_string(path) {
                    Ok(t) => t,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(_) => return Err("Профиль нельзя прочитать без потери кодировки".into()),
                };
                Ok(shell_range(&text)?.map(|r| json!(&text[r])))
            }
            _ => Err("Неверный файловый ресурс".into()),
        }
    }
    fn write(&mut self, resource: &Resource, value: &Option<Value>) -> Result<(), String> {
        match resource {
            Resource::Json { path, keys } => {
                let text = read_text(path)?;
                let updated = edit_key(&text, keys, value.as_ref().and_then(Value::as_str))?;
                atomic_write(path, updated.as_bytes())
            }
            Resource::Shell(path) => {
                let mut text = match fs::read_to_string(path) {
                    Ok(t) => t,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                    Err(_) => return Err("Профиль нельзя прочитать без потери кодировки".into()),
                };
                let replacement = value.as_ref().and_then(Value::as_str).unwrap_or("");
                if let Some(range) = shell_range(&text)? {
                    text.replace_range(range, replacement);
                } else if !replacement.is_empty() {
                    // Не добавляем символы вне собственных маркеров: удаление возвращает исходные байты.
                    if !text.is_empty() && !text.ends_with('\n') && text != "\u{feff}" {
                        return Err(
                            "Профиль без конечного перевода строки: безопасная вставка отменена"
                                .into(),
                        );
                    }
                    text.push_str(replacement);
                }
                atomic_write(path, text.as_bytes())
            }
            _ => Err("Неверный файловый ресурс".into()),
        }
    }
}

pub(super) fn apply_json(path: &Path, changes: Vec<(Vec<String>, Value)>) -> Result<bool, String> {
    let mut journal = Journal::open(default_path()?)?;
    let mut files = Files;
    let mut changed = false;
    for (keys, value) in changes {
        for depth in 1..keys.len() {
            let parent = Resource::Json {
                path: path.to_path_buf(),
                keys: keys[..depth].to_vec(),
            };
            if files.read(&parent)?.is_none() {
                changed |= journal.apply(&mut files, parent, Some(json!("{}")))?;
            }
        }
        changed |= journal.apply(
            &mut files,
            Resource::Json {
                path: path.to_path_buf(),
                keys,
            },
            Some(json!(
                serde_json::to_string(&value).map_err(|_| "Неверное значение")?
            )),
        )?;
    }
    Ok(changed)
}

pub(super) fn cleanup_json(path: &Path) -> Result<bool, String> {
    let mut journal = Journal::open(default_path()?)?;
    let mut changed = false;
    let mut conflict = false;
    for resource in journal.resources(false).into_iter().rev() {
        if matches!(&resource, Resource::Json { path: p, .. } if p == path) {
            match journal.restore_one(&mut Files, &resource)? {
                Outcome::Restored => changed = true,
                Outcome::Conflict => conflict = true,
                _ => {}
            }
        }
    }
    if conflict {
        return Err("Внешнее изменение ключа сохранено".into());
    }
    Ok(changed)
}

pub(super) fn apply_shell(path: &Path, block: String) -> Result<bool, String> {
    let mut journal = Journal::open(default_path()?)?;
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return Err("Кодировка профиля не поддерживается; файл сохранён".into()),
    };
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let block = format!("{}\n", block.trim_end_matches(['\r', '\n'])).replace('\n', newline);
    // Старый опасный блок в точных маркерах принадлежит Zastava, его исходник не восстанавливается.
    if let Some(range) = shell_range(&text)? {
        if text[range.clone()].contains("Remove-Item Env:") {
            let mut safe = text.clone();
            safe.replace_range(range, "");
            atomic_write(path, safe.as_bytes())?;
        }
    }
    journal.apply(
        &mut Files,
        Resource::Shell(path.to_path_buf()),
        Some(json!(block)),
    )
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    fn parsed(path: &Path) -> Value {
        serde_json::from_str(&sanitize_jsonc(&fs::read_to_string(path).unwrap())).unwrap()
    }

    #[test]
    fn t15_jsonc_restores_each_preexisting_value_and_keeps_comments_crlf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let initial = "{\r\n // мой комментарий\r\n \"http.proxy\": \"user-proxy\",\r\n \"http.proxySupport\": \"on\",\r\n \"env\": {\"TZ\":\"user-tz\",\"LANG\":\"ru_RU\",\"TOKEN\":\"secret\"},\r\n}\r\n";
        fs::write(&path, initial).unwrap();
        apply_claude_code_settings_file_with_tz(
            &path,
            "http://127.0.0.1:8899",
            Some("Europe/Paris"),
        )
        .unwrap();
        cleanup_claude_code_settings_file(&path).unwrap();
        let result = parsed(&path);
        assert_eq!(
            result["env"],
            json!({"TZ":"user-tz","LANG":"ru_RU","TOKEN":"secret"})
        );
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("// мой комментарий\r\n"));
        assert_eq!(text.matches('\n').count(), text.matches("\r\n").count());
        assert_eq!(result["http.proxy"], "user-proxy");
    }

    #[test]
    fn t15_ide_existing_proxy_timezone_and_neighbors_restored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let env_key = if cfg!(target_os = "macos") {
            "terminal.integrated.env.osx"
        } else if cfg!(target_os = "windows") {
            "terminal.integrated.env.windows"
        } else {
            "terminal.integrated.env.linux"
        };
        let initial = json!({"http.proxy":"vpn","http.proxySupport":"on",env_key:{"TZ":"old","LANG":"ru","OTHER":42,"HTTP_PROXY":null,"NO_PROXY":""}});
        fs::write(&path, serde_json::to_vec(&initial).unwrap()).unwrap();
        apply_ide_settings_file_with_tz(&path, "http://127.0.0.1:8899", Some("Europe/Paris"))
            .unwrap();
        cleanup_ide_settings_file(&path).unwrap();
        assert_eq!(parsed(&path), initial);
    }

    #[test]
    fn t16_user_changed_tz_proxy_and_sibling_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{}").unwrap();
        apply_claude_code_settings_file_with_tz(
            &path,
            "http://127.0.0.1:8899",
            Some("Europe/Paris"),
        )
        .unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let raw = edit_key(&raw, &["env".into(), "TZ".into()], Some("\"user-changed\"")).unwrap();
        let raw = edit_key(
            &raw,
            &["env".into(), "HTTP_PROXY".into()],
            Some("\"throne\""),
        )
        .unwrap();
        let raw = edit_key(&raw, &["env".into(), "CUSTOM".into()], Some("true")).unwrap();
        fs::write(&path, raw).unwrap();
        assert!(cleanup_claude_code_settings_file(&path).is_err());
        assert_eq!(
            parsed(&path)["env"],
            json!({"TZ":"user-changed","HTTP_PROXY":"throne","CUSTOM":true})
        );
    }

    #[test]
    fn t17_bom_unicode_urls_strings_and_comments_survive_edits() {
        let input = "\u{feff}{\r\n // комментарий\r\n \"url\":\"https://a//b/*c*/\",\r\n \"nested\":{\"message\":\"Привет \\\"мир\\\"\",},\r\n}\r\n";
        let changed = edit_key(input, &["new".into()], Some("42")).unwrap();
        let restored = edit_key(&changed, &["new".into()], None).unwrap();
        assert!(restored.starts_with('\u{feff}'));
        assert!(restored.contains(" // комментарий\r\n"));
        assert!(restored.contains("\"url\":\"https://a//b/*c*/\""));
        assert!(restored.contains("Привет"));
    }

    #[test]
    fn malformed_duplicate_and_nonobject_configs_are_never_overwritten() {
        for raw in [
            "{bad",
            "{\"env\":\"user\"}",
            "{\"env\":{},\"env\":{}}",
            "[]",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            fs::write(&path, raw).unwrap();
            assert!(
                apply_claude_code_settings_file_with_tz(
                    &path,
                    "http://127.0.0.1:8899",
                    Some("Europe/Paris")
                )
                .is_err()
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), raw);
        }
    }

    #[test]
    fn user_comment_inside_created_empty_object_prevents_container_removal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{}").unwrap();
        apply_claude_code_settings_file(&path, "http://127.0.0.1:8899").unwrap();
        let raw = fs::read_to_string(&path).unwrap().replacen(
            "\"env\": {",
            "\"env\": { /* комментарий пользователя */",
            1,
        );
        fs::write(&path, raw).unwrap();
        assert!(cleanup_claude_code_settings_file(&path).is_err());
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("комментарий пользователя")
        );
    }

    #[test]
    fn t21_no_journal_never_cleans_user_localhost_or_tz() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let raw = "{\"env\":{\"HTTPS_PROXY\":\"http://127.0.0.1:8899\",\"TZ\":\"user\"}}";
        fs::write(&path, raw).unwrap();
        assert!(!cleanup_claude_code_settings_file(&path).unwrap());
        assert_eq!(fs::read_to_string(path).unwrap(), raw);
    }

    #[test]
    fn t14_shell_keeps_bytes_bom_crlf_permissions_and_external_append() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.ps1");
        let original = "\u{feff}# Пользовательский профиль\r\n$env:TZ='user'\r\n";
        fs::write(&path, original).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        }
        apply_shell(
            &path,
            build_powershell_guard_block_with_tz("http://127.0.0.1:8899", Some("Europe/Paris")),
        )
        .unwrap();
        let mut modified = fs::read_to_string(&path).unwrap();
        modified.push_str("# Добавлено пользователем\r\n");
        fs::write(&path, modified).unwrap();
        let mut j = Journal::open(default_path().unwrap()).unwrap();
        let reports = j.restore(&mut Files, false);
        assert!(reports.iter().all(|r| r.outcome == Outcome::Restored));
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!("{original}# Добавлено пользователем\r\n")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o640
            );
        }
    }

    #[test]
    fn shell_edited_owned_block_is_not_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".zshrc");
        fs::write(&path, "# user\n").unwrap();
        apply_shell(
            &path,
            build_shell_rc_guard_block("http://127.0.0.1:8899", 8899),
        )
        .unwrap();
        let edited = fs::read_to_string(&path)
            .unwrap()
            .replace("nc -z", "nc -w 1 -z");
        fs::write(&path, &edited).unwrap();
        let mut j = Journal::open(default_path().unwrap()).unwrap();
        assert_eq!(j.restore(&mut Files, false)[0].outcome, Outcome::Conflict);
        assert_eq!(fs::read_to_string(path).unwrap(), edited);
    }

    #[test]
    fn t12_t13_old_powershell_dangerous_block_is_migrated_and_not_restored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.ps1");
        let prefix = "$env:TZ='user'\r\n";
        fs::write(
            &path,
            format!("{prefix}{ZASTAVA_RC_START}\r\nRemove-Item Env:TZ\r\n{ZASTAVA_RC_END}\r\n"),
        )
        .unwrap();
        apply_shell(
            &path,
            build_powershell_guard_block_with_tz("http://127.0.0.1:8899", None),
        )
        .unwrap();
        assert!(!fs::read_to_string(&path).unwrap().contains("Remove-Item"));
        let mut j = Journal::open(default_path().unwrap()).unwrap();
        j.restore(&mut Files, false);
        assert_eq!(fs::read_to_string(path).unwrap(), prefix);
    }

    #[test]
    fn partial_markers_non_utf8_and_unterminated_profile_are_preserved() {
        for bytes in [
            b"\xff\xfe\x00\x01".as_slice(),
            b"# user without newline",
            ZASTAVA_RC_START.as_bytes(),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("profile");
            fs::write(&path, bytes).unwrap();
            assert!(
                apply_shell(
                    &path,
                    build_shell_rc_guard_block("http://127.0.0.1:8899", 8899)
                )
                .is_err()
            );
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        assert_eq!(
            remove_shell_rc_block("echo '# >>> Zastava AI CLI & IDE Guard >>>'\n"),
            "echo '# >>> Zastava AI CLI & IDE Guard >>>'\n"
        );
    }

    #[test]
    fn t04_t07_installer_and_environment_have_no_unowned_reset() {
        let hooks = include_str!("../../windows/hooks.nsh");
        for token in ["DeleteRegValue", "WriteRegDWORD", "WM_WININICHANGE"] {
            assert!(!hooks.contains(token));
        }
        assert!(hooks.contains("--cleanup"));
        assert!(hooks.contains("ExecWait"));
        let orchestration = include_str!("cli_protection.rs");
        assert!(!orchestration.contains("\"setenv\""));
        assert!(!orchestration.contains("\"unsetenv\""));
        let block =
            build_powershell_guard_block_with_tz("http://127.0.0.1:8899", Some("Europe/Paris"));
        assert!(!block.contains("Remove-Item"));
        assert!(!block.contains("else {"));
        // Функция миграции глобального Environment намеренно пуста.
        cleanup_legacy_windows_user_env();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_target_is_not_replaced() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let path = dir.path().join("profile");
        fs::write(&target, "# user\n").unwrap();
        symlink(&target, &path).unwrap();
        assert!(
            apply_shell(
                &path,
                build_shell_rc_guard_block("http://127.0.0.1:8899", 8899)
            )
            .is_err()
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "# user\n");
    }
}
