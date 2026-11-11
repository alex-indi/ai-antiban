//! SystemConfiguration: отдельные свойства по стабильному ID сетевого сервиса.
//! В отличие от networksetup API сохраняет отсутствие ключа и исходный тип plist.
use super::owned_settings::{Backend, Journal, Resource, default_path};
use super::{is_gateway_host_port, parse_gateway_port_from_pac_url, set_chained_proxy};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    ffi::{CString, c_void},
    ptr,
};

type CF = *const c_void;
type MutableCF = *mut c_void;
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: CF);
    fn CFStringCreateWithCString(alloc: CF, text: *const i8, encoding: u32) -> CF;
    fn CFStringGetCString(text: CF, buffer: *mut i8, size: isize, encoding: u32) -> bool;
    fn CFArrayGetCount(array: CF) -> isize;
    fn CFArrayGetValueAtIndex(array: CF, index: isize) -> CF;
    fn CFDictionaryGetValue(dictionary: CF, key: CF) -> CF;
    fn CFDictionaryGetCount(dictionary: CF) -> isize;
    fn CFDictionaryGetKeysAndValues(dictionary: CF, keys: *mut CF, values: *mut CF);
    fn CFGetTypeID(value: CF) -> usize;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFDictionaryCreateMutableCopy(alloc: CF, size: isize, dictionary: CF) -> MutableCF;
    fn CFDictionarySetValue(dictionary: MutableCF, key: CF, value: CF);
    fn CFDictionaryRemoveValue(dictionary: MutableCF, key: CF);
    fn CFPropertyListCreateData(
        alloc: CF,
        value: CF,
        format: isize,
        options: usize,
        error: *mut CF,
    ) -> CF;
    fn CFPropertyListCreateWithData(
        alloc: CF,
        data: CF,
        options: usize,
        format: *mut isize,
        error: *mut CF,
    ) -> CF;
    fn CFDataCreate(alloc: CF, bytes: *const u8, length: isize) -> CF;
    fn CFDataGetLength(data: CF) -> isize;
    fn CFDataGetBytePtr(data: CF) -> *const u8;
}
#[link(name = "SystemConfiguration", kind = "framework")]
unsafe extern "C" {
    fn SCPreferencesCreateWithAuthorization(
        alloc: CF,
        name: CF,
        id: CF,
        authorization: MutableCF,
    ) -> CF;
    fn SCPreferencesLock(preferences: CF, wait: bool) -> bool;
    fn SCPreferencesUnlock(preferences: CF) -> bool;
    fn SCPreferencesCommitChanges(preferences: CF) -> bool;
    fn SCPreferencesApplyChanges(preferences: CF) -> bool;
    fn SCDynamicStoreCreate(alloc: CF, name: CF, callback: CF, context: MutableCF) -> CF;
    fn SCDynamicStoreCopyKeyList(store: CF, pattern: CF) -> CF;
    fn SCDynamicStoreCopyValue(store: CF, key: CF) -> CF;
    fn SCNetworkServiceCopyAll(preferences: CF) -> CF;
    fn SCNetworkServiceGetServiceID(service: CF) -> CF;
    fn SCNetworkServiceGetEnabled(service: CF) -> bool;
    fn SCNetworkServiceGetName(service: CF) -> CF;
    fn SCNetworkServiceCopyProtocol(service: CF, kind: CF) -> CF;
    fn SCNetworkProtocolGetConfiguration(protocol: CF) -> CF;
    fn SCNetworkProtocolSetConfiguration(protocol: CF, configuration: CF) -> bool;
}
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn AuthorizationCreate(
        rights: CF,
        environment: CF,
        flags: u32,
        authorization: *mut MutableCF,
    ) -> i32;
    fn AuthorizationFree(authorization: MutableCF, flags: u32) -> i32;
}

struct Owned(CF);
impl Owned {
    fn new(value: CF) -> Result<Self, String> {
        if value.is_null() {
            Err(failure())
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0);
        }
    }
}
fn failure() -> String {
    "SystemConfiguration не подтвердил операцию; журнал сохранён для повторного восстановления"
        .into()
}
fn string(text: &str) -> Result<Owned, String> {
    let text = CString::new(text).map_err(|_| failure())?;
    Owned::new(unsafe { CFStringCreateWithCString(ptr::null(), text.as_ptr(), 0x08000100) })
}
fn text(value: CF) -> Result<String, String> {
    if value.is_null() {
        return Err(failure());
    }
    let mut buffer = vec![0u8; 4096];
    if !unsafe {
        CFStringGetCString(
            value,
            buffer.as_mut_ptr().cast(),
            buffer.len() as isize,
            0x08000100,
        )
    } {
        return Err(failure());
    }
    let end = buffer.iter().position(|b| *b == 0).ok_or_else(failure)?;
    String::from_utf8(buffer[..end].to_vec()).map_err(|_| failure())
}
struct Authorization(MutableCF);
impl Drop for Authorization {
    fn drop(&mut self) {
        unsafe {
            AuthorizationFree(self.0, 0);
        }
    }
}
struct Preferences {
    preferences: Owned,
    _authorization: Authorization,
}
impl Preferences {
    fn new() -> Result<Self, String> {
        let mut authorization = ptr::null_mut();
        if unsafe { AuthorizationCreate(ptr::null(), ptr::null(), 0, &mut authorization) } != 0 {
            return Err(failure());
        }
        let authorization = Authorization(authorization);
        let name = string("Zastava: безопасное управление прокси")?;
        let preferences = Owned::new(unsafe {
            SCPreferencesCreateWithAuthorization(ptr::null(), name.0, ptr::null(), authorization.0)
        });
        match preferences {
            Ok(preferences) => Ok(Self {
                preferences,
                _authorization: authorization,
            }),
            Err(e) => Err(e),
        }
    }
    fn services(&self) -> Result<Owned, String> {
        Owned::new(unsafe { SCNetworkServiceCopyAll(self.preferences.0) })
    }
    fn protocol(&self, id: &str) -> Result<Owned, String> {
        let services = self.services()?;
        for index in 0..unsafe { CFArrayGetCount(services.0) } {
            let service = unsafe { CFArrayGetValueAtIndex(services.0, index) };
            if text(unsafe { SCNetworkServiceGetServiceID(service) })? == id {
                return Owned::new(unsafe {
                    SCNetworkServiceCopyProtocol(service, string("Proxies")?.0)
                });
            }
        }
        Err("Сетевой сервис удалён; его настройки не изменены".into())
    }
}

pub(super) fn refresh_preferences() -> Result<(), String> {
    let prefs = Preferences::new()?;
    ensure_dns_can_be_reapplied(&prefs)?;
    if !unsafe { SCPreferencesApplyChanges(prefs.preferences.0) } {
        return Err(failure());
    }
    Ok(())
}

fn serialize(value: CF) -> Result<Option<Value>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let data = Owned::new(unsafe {
        CFPropertyListCreateData(ptr::null(), value, 100, 0, ptr::null_mut())
    })?;
    let bytes = unsafe {
        std::slice::from_raw_parts(CFDataGetBytePtr(data.0), CFDataGetLength(data.0) as usize)
    };
    Ok(Some(json!(STANDARD.encode(bytes))))
}
fn deserialize(value: &Value) -> Result<Owned, String> {
    let bytes = STANDARD
        .decode(value.as_str().ok_or_else(failure)?)
        .map_err(|_| failure())?;
    let data =
        Owned::new(unsafe { CFDataCreate(ptr::null(), bytes.as_ptr(), bytes.len() as isize) })?;
    Owned::new(unsafe {
        CFPropertyListCreateWithData(ptr::null(), data.0, 0, ptr::null_mut(), ptr::null_mut())
    })
}
fn plist(body: &str) -> Value {
    json!(STANDARD.encode(format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">{body}</plist>\n")))
}
fn property(config: CF, key: &str) -> Result<Option<Value>, String> {
    serialize(unsafe { CFDictionaryGetValue(config, string(key)?.0) })
}

/// configd при ApplyChanges заново строит весь Setup, включая DNS других программ.
/// Нельзя применять даже PAC неактивного сервиса поверх временных DNS VPN.
fn dns_signature(dictionary: CF) -> Result<Option<Value>, String> {
    if dictionary.is_null() {
        return Ok(None);
    }
    if unsafe { CFGetTypeID(dictionary) != CFDictionaryGetTypeID() } {
        return Err("Неизвестный формат DNS: системный PAC не изменён".into());
    }
    let count =
        usize::try_from(unsafe { CFDictionaryGetCount(dictionary) }).map_err(|_| failure())?;
    if count > 1024 {
        return Err("Не удалось безопасно проверить временные DNS".into());
    }
    let mut keys = vec![ptr::null(); count];
    let mut values = vec![ptr::null(); count];
    unsafe {
        CFDictionaryGetKeysAndValues(dictionary, keys.as_mut_ptr(), values.as_mut_ptr());
    }
    let mut fields = serde_json::Map::new();
    for (key, value) in keys.into_iter().zip(values) {
        if unsafe { CFGetTypeID(key) != CFStringGetTypeID() } {
            return Err("Неизвестный ключ DNS: системный PAC не изменён".into());
        }
        let key = text(key)?;
        // Служебные поля configd не являются пользовательскими DNS-настройками.
        if !key.starts_with("__") {
            fields.insert(key, serialize(value)?.ok_or_else(failure)?);
        }
    }
    Ok((!fields.is_empty()).then_some(Value::Object(fields)))
}

fn ensure_dns_matches(stored: &Option<Value>, setup: &Option<Value>) -> Result<(), String> {
    if stored != setup {
        return Err("Обнаружены временные DNS VPN: применение системного PAC может их сбросить. Настройки не изменены".into());
    }
    Ok(())
}

fn ensure_dns_can_be_reapplied(prefs: &Preferences) -> Result<(), String> {
    let name = string("Zastava: проверка временных DNS")?;
    let store = Owned::new(unsafe {
        SCDynamicStoreCreate(ptr::null(), name.0, ptr::null(), ptr::null_mut())
    })?;
    let pattern = string("^Setup:/Network/Service/[^/]+/DNS$")?;
    let keys = Owned::new(unsafe { SCDynamicStoreCopyKeyList(store.0, pattern.0) })?;
    let services = prefs.services()?;
    for index in 0..unsafe { CFArrayGetCount(keys.0) } {
        let key = unsafe { CFArrayGetValueAtIndex(keys.0, index) };
        let path = text(key)?;
        let id = path
            .strip_prefix("Setup:/Network/Service/")
            .and_then(|p| p.strip_suffix("/DNS"))
            .ok_or_else(failure)?;
        let setup = Owned::new(unsafe { SCDynamicStoreCopyValue(store.0, key) })?;
        let mut stored = None;
        for index in 0..unsafe { CFArrayGetCount(services.0) } {
            let service = unsafe { CFArrayGetValueAtIndex(services.0, index) };
            if text(unsafe { SCNetworkServiceGetServiceID(service) })? == id {
                let protocol = unsafe { SCNetworkServiceCopyProtocol(service, string("DNS")?.0) };
                if !protocol.is_null() {
                    let protocol = Owned::new(protocol)?;
                    stored =
                        dns_signature(unsafe { SCNetworkProtocolGetConfiguration(protocol.0) })?;
                }
                break;
            }
        }
        ensure_dns_matches(&stored, &dns_signature(setup.0)?)?;
    }
    Ok(())
}

pub(super) fn read(resource: &Resource) -> Result<Option<Value>, String> {
    let Resource::MacProxy { service, key } = resource else {
        return Err(failure());
    };
    let prefs = Preferences::new()?;
    let protocol = prefs.protocol(service)?;
    let config = unsafe { SCNetworkProtocolGetConfiguration(protocol.0) };
    if config.is_null() {
        return Err("Нет конфигурации Proxies; сервис не изменён".into());
    }
    let raw = property(config, key)?;
    if key == "ProxyAutoConfigEnable" {
        Ok(Some(
            json!({"raw": raw, "url": property(config, "ProxyAutoConfigURLString")?}),
        ))
    } else if key == "HTTPEnable" || key == "HTTPSEnable" {
        let prefix = key.trim_end_matches("Enable");
        Ok(Some(
            json!({"raw": raw, "host": property(config, &format!("{prefix}Proxy"))?, "port": property(config, &format!("{prefix}Port"))?, "user": property(config, &format!("{prefix}User"))?}),
        ))
    } else {
        Ok(raw)
    }
}

pub(super) fn write(resource: &Resource, desired: &Option<Value>) -> Result<(), String> {
    let Resource::MacProxy { service, key } = resource else {
        return Err(failure());
    };
    let prefs = Preferences::new()?;
    // До захвата прав и любой записи отказываемся от глобального ApplyChanges,
    // если он сотрёт временную DNS-конфигурацию другого приложения.
    ensure_dns_can_be_reapplied(&prefs)?;
    if !unsafe { SCPreferencesLock(prefs.preferences.0, false) } {
        return Err(failure());
    }
    let result = (|| {
        ensure_dns_can_be_reapplied(&prefs)?;
        let protocol = prefs.protocol(service)?;
        let config = unsafe { SCNetworkProtocolGetConfiguration(protocol.0) };
        if config.is_null() {
            return Err(failure());
        }
        let copy = Owned::new(unsafe { CFDictionaryCreateMutableCopy(ptr::null(), 0, config) })?;
        let raw = if matches!(
            key.as_str(),
            "HTTPEnable" | "HTTPSEnable" | "ProxyAutoConfigEnable"
        ) {
            desired
                .as_ref()
                .and_then(|v| v.get("raw"))
                .filter(|v| !v.is_null())
        } else {
            desired.as_ref()
        };
        let key = string(key)?;
        if let Some(raw) = raw {
            let value = deserialize(raw)?;
            unsafe {
                CFDictionarySetValue(copy.0.cast_mut(), key.0, value.0);
            }
        } else {
            unsafe {
                CFDictionaryRemoveValue(copy.0.cast_mut(), key.0);
            }
        }
        if !unsafe { SCNetworkProtocolSetConfiguration(protocol.0, copy.0) }
            || !unsafe { SCPreferencesCommitChanges(prefs.preferences.0) }
            || !unsafe { SCPreferencesApplyChanges(prefs.preferences.0) }
        {
            return Err(failure());
        }
        Ok(())
    })();
    if !unsafe { SCPreferencesUnlock(prefs.preferences.0) } {
        return Err(failure());
    }
    result
}

fn is_enabled(value: &Value) -> bool {
    value
        .as_str()
        .and_then(|s| STANDARD.decode(s).ok())
        .is_some_and(|bytes| {
            let xml = String::from_utf8_lossy(&bytes);
            xml.contains("<integer>1</integer>") || xml.contains("<true/>")
        })
}
fn proxy_text(value: &Value) -> Option<String> {
    let bytes = STANDARD.decode(value.as_str()?).ok()?;
    let xml = String::from_utf8(bytes).ok()?;
    let body = xml.split_once("<string>")?.1.split_once("</string>")?.0;
    // Адреса с XML entities не передаются генератору PAC без точного декодирования.
    if body.contains('&') {
        return None;
    }
    Some(body.into())
}
fn proxy_port(value: &Value) -> Option<u16> {
    let bytes = STANDARD.decode(value.as_str()?).ok()?;
    let xml = String::from_utf8(bytes).ok()?;
    xml.split_once("<integer>")?
        .1
        .split_once("</integer>")?
        .0
        .parse()
        .ok()
}

pub(super) fn enable_system_proxy(
    pac_url: &str,
    configured: &mut Vec<String>,
) -> Result<(), String> {
    // URL генерирует ядро, но XML-экранирование всё равно обязательно.
    let escaped = pac_url
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let normalize = |value: Value| -> Result<Value, String> {
        serialize(deserialize(&value)?.0)?.ok_or_else(failure)
    };
    let desired_url = normalize(plist(&format!("<string>{escaped}</string>")))?;
    let one = normalize(plist("<integer>1</integer>"))?;
    let zero = normalize(plist("<integer>0</integer>"))?;
    let prefs = Preferences::new()?;
    ensure_dns_can_be_reapplied(&prefs)?;
    let services = prefs.services()?;
    let mut journal = Journal::open(default_path()?)?;
    let mut backend = super::SystemSettings;
    let continuing = !journal.resources(true).is_empty();
    let mut changes = vec![];
    let mut targets = vec![];
    // Снимки всех сервисов проверяются до первой записи.
    for index in 0..unsafe { CFArrayGetCount(services.0) } {
        let service = unsafe { CFArrayGetValueAtIndex(services.0, index) };
        if !unsafe { SCNetworkServiceGetEnabled(service) } {
            continue;
        }
        let id = text(unsafe { SCNetworkServiceGetServiceID(service) })?;
        let name = text(unsafe { SCNetworkServiceGetName(service) })?;
        if prefs.protocol(&id).is_err() {
            continue;
        }
        for key in ["HTTPEnable", "HTTPSEnable"] {
            let resource = Resource::MacProxy {
                service: id.clone(),
                key: key.into(),
            };
            let mut baseline = journal
                .original(&resource)
                .unwrap_or(backend.read(&resource)?)
                .ok_or_else(failure)?;
            if is_enabled(&baseline["raw"]) {
                if baseline.get("user").is_some_and(|v| !v.is_null()) {
                    return Err("Proxy с аутентификацией нельзя безопасно включить в PAC".into());
                }
                let host =
                    proxy_text(&baseline["host"]).ok_or("Неизвестный формат адреса proxy")?;
                let port = proxy_port(&baseline["port"]).ok_or("Неизвестный формат порта proxy")?;
                let target = format!("{host}:{port}");
                if is_gateway_host_port(&target, parse_gateway_port_from_pac_url(pac_url)) {
                    return Err("Петля proxy: системные настройки сохранены".into());
                }
                targets.push(target);
                baseline["raw"] = zero.clone();
                changes.push((resource, baseline));
            }
        }
        changes.push((
            Resource::MacProxy {
                service: id.clone(),
                key: "ProxyAutoConfigURLString".into(),
            },
            desired_url.clone(),
        ));
        changes.push((
            Resource::MacProxy {
                service: id,
                key: "ProxyAutoConfigEnable".into(),
            },
            json!({"raw": one, "url": desired_url}),
        ));
        configured.push(name);
    }
    targets.sort();
    targets.dedup();
    if targets.len() > 1 {
        return Err("Различные HTTP/HTTPS upstream нельзя объединить в существующий PAC без потери маршрутизации".into());
    }
    if configured.is_empty() {
        return Err("Не найдено сетевых сервисов с конфигурацией Proxies".into());
    }
    if let Some(target) = targets.pop() {
        set_chained_proxy(Some(target));
    }
    for (resource, value) in &changes {
        journal.check(&mut backend, resource)?;
        if continuing
            && journal.original(resource).is_none()
            && backend.read(resource)? != Some(value.clone())
        {
            return Err(
                "Новый сервис или внешнее изменение не захватывается при повторном включении"
                    .into(),
            );
        }
    }
    for (resource, value) in changes {
        if let Resource::MacProxy { service, key } = &resource {
            if key == "ProxyAutoConfigURLString" {
                let flag = Resource::MacProxy {
                    service: service.clone(),
                    key: "ProxyAutoConfigEnable".into(),
                };
                let mut after = backend.read(&flag)?.ok_or_else(failure)?;
                after["url"] = value.clone();
                journal.prepare_context(&mut backend, &flag, Some(after))?;
            }
        }
        journal.apply(&mut backend, resource, Some(value))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dns(body: &str) -> Option<Value> {
        dns_signature(deserialize(&plist(body)).unwrap().0).unwrap()
    }

    #[test]
    fn temporary_dns_override_is_rejected_without_exposing_values() {
        let live = dns(
            "<dict><key>ServerAddresses</key><array><string>vpn-private-value</string></array></dict>",
        );
        let error = ensure_dns_matches(&None, &live).unwrap_err();
        assert!(error.contains("временные DNS"));
        assert!(!error.contains("vpn-private-value"));
    }

    #[test]
    fn stored_dns_and_empty_metadata_are_supported() {
        let saved = dns(
            "<dict><key>ServerAddresses</key><array><string>192.0.2.53</string></array></dict>",
        );
        assert!(ensure_dns_matches(&saved, &saved).is_ok());
        assert_eq!(dns("<dict/>"), None);
        assert_eq!(
            dns("<dict><key>__FLAGS__</key><integer>2</integer></dict>"),
            None
        );
        assert!(ensure_dns_matches(&None, &None).is_ok());
    }

    #[test]
    fn dns_dictionary_order_does_not_create_a_conflict() {
        let a = dns(
            "<dict><key>SearchDomains</key><array><string>example.test</string></array><key>ServerAddresses</key><array><string>192.0.2.53</string></array></dict>",
        );
        let b = dns(
            "<dict><key>ServerAddresses</key><array><string>192.0.2.53</string></array><key>SearchDomains</key><array><string>example.test</string></array></dict>",
        );
        assert!(ensure_dns_matches(&a, &b).is_ok());
    }

    #[test]
    fn unknown_dns_fields_are_preserved_by_refusing_apply() {
        let stored = dns("<dict><key>Options</key><string>old</string></dict>");
        let changed = dns(
            "<dict><key>Options</key><string>old</string><key>FutureVPNField</key><true/></dict>",
        );
        assert!(ensure_dns_matches(&stored, &changed).is_err());
        assert!(ensure_dns_matches(&stored, &None).is_err());
    }

    // Только преобразование данных CoreFoundation; Preferences/сеть не открываются.
    #[test]
    fn plist_types_and_empty_string_roundtrip_without_system_access() {
        for body in [
            "<integer>0</integer>",
            "<integer>1</integer>",
            "<string></string>",
            "<true/>",
            "<string>http://127.0.0.1:1080</string>",
        ] {
            let first = serialize(deserialize(&plist(body)).unwrap().0).unwrap();
            let second = serialize(deserialize(first.as_ref().unwrap()).unwrap().0).unwrap();
            assert_eq!(first, second);
        }
        assert_eq!(serialize(ptr::null()).unwrap(), None);
        let value = serialize(deserialize(&plist("<integer>1</integer>")).unwrap().0)
            .unwrap()
            .unwrap();
        assert!(is_enabled(&value));
    }
}
