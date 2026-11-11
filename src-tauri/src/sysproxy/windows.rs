//! WinINet: точные типы и байты реестра, восстановление с проверкой владельца.
use super::{
    owned_settings::{Backend, Journal, Resource, default_path},
    parse_gateway_port_from_pac_url, parse_windows_proxy_server, set_chained_proxy,
};
use serde_json::{Value, json};
use std::io;
use winreg::{RegKey, RegValue, enums::*};

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

fn failure() -> String {
    "Ошибка доступа к WinINet; неизвестные настройки не изменены".into()
}
fn read_raw(name: &str) -> Result<Option<Value>, String> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(KEY, KEY_READ)
        .map_err(|_| failure())?;
    match key.get_raw_value(name) {
        Ok(v) => Ok(Some(json!({"kind": v.vtype as u32, "bytes": v.bytes}))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(failure()),
    }
}

pub(super) fn read(resource: &Resource) -> Result<Option<Value>, String> {
    let Resource::WindowsProxy(name) = resource else {
        return Err(failure());
    };
    let raw = read_raw(name)?;
    if name == "ProxyEnable" {
        // Не включаем старый explicit proxy, если VPN сменил его адрес или исключения.
        Ok(Some(
            json!({"raw": raw, "server": read_raw("ProxyServer")?, "bypass": read_raw("ProxyOverride")?, "pac": read_raw("AutoConfigURL")?}),
        ))
    } else {
        Ok(raw)
    }
}

pub(super) fn write(resource: &Resource, value: &Option<Value>) -> Result<(), String> {
    let Resource::WindowsProxy(name) = resource else {
        return Err(failure());
    };
    let value = if name == "ProxyEnable" {
        value
            .as_ref()
            .and_then(|v| v.get("raw"))
            .filter(|v| !v.is_null())
    } else {
        value.as_ref()
    };
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(KEY, KEY_SET_VALUE)
        .map_err(|_| failure())?;
    if let Some(value) = value {
        let kind = match value["kind"].as_u64() {
            Some(0) => REG_NONE,
            Some(1) => REG_SZ,
            Some(2) => REG_EXPAND_SZ,
            Some(3) => REG_BINARY,
            Some(4) => REG_DWORD,
            Some(5) => REG_DWORD_BIG_ENDIAN,
            Some(6) => REG_LINK,
            Some(7) => REG_MULTI_SZ,
            Some(8) => REG_RESOURCE_LIST,
            Some(9) => REG_FULL_RESOURCE_DESCRIPTOR,
            Some(10) => REG_RESOURCE_REQUIREMENTS_LIST,
            Some(11) => REG_QWORD,
            _ => return Err(failure()),
        };
        let bytes: Vec<u8> =
            serde_json::from_value(value["bytes"].clone()).map_err(|_| failure())?;
        key.set_raw_value(
            name,
            &RegValue {
                vtype: kind,
                bytes: bytes.into(),
            },
        )
        .map_err(|_| failure())?;
    } else {
        match key.delete_value(name) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(failure()),
        }
    }
    notify_windows_inet_refresh()?;
    Ok(())
}

fn decode_string(value: &Value) -> Option<String> {
    if value["kind"] != 1 {
        return None;
    }
    let bytes: Vec<u8> = serde_json::from_value(value["bytes"].clone()).ok()?;
    if bytes.len() % 2 != 0 {
        return None;
    }
    let mut words: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    if words.last() == Some(&0) {
        words.pop();
    }
    String::from_utf16(&words).ok()
}

pub(super) fn enable_system_proxy(
    pac_url: &str,
    configured: &mut Vec<String>,
) -> Result<(), String> {
    let mut journal = Journal::open(default_path()?)?;
    let mut system = super::SystemSettings;
    let flag = Resource::WindowsProxy("ProxyEnable".into());
    let pac = Resource::WindowsProxy("AutoConfigURL".into());
    let baseline = journal
        .original(&flag)
        .unwrap_or(system.read(&flag)?)
        .ok_or_else(failure)?;
    let raw = &baseline["raw"];
    let bytes: Vec<u8> = serde_json::from_value(raw["bytes"].clone()).unwrap_or_default();
    if raw["kind"] == 4 && bytes == 1u32.to_le_bytes() {
        if let Some(server) = baseline.get("server").and_then(decode_string) {
            // Существующий генератор PAC поддерживает единственный HTTP upstream.
            let endpoints: Vec<&str> = server
                .split(';')
                .map(|s| s.split_once('=').map_or(s, |(_, v)| v).trim())
                .collect();
            if endpoints.windows(2).any(|w| w[0] != w[1])
                || server.to_ascii_lowercase().contains("socks=")
            {
                return Err(
                    "Различные proxy для протоколов нельзя безопасно объединить в один PAC".into(),
                );
            }
            let target =
                parse_windows_proxy_server(&server, parse_gateway_port_from_pac_url(pac_url))
                    .ok_or("Не удалось безопасно построить цепочку прокси")?;
            set_chained_proxy(Some(target));
        } else {
            return Err("Неизвестный тип ProxyServer: настройки сохранены".into());
        }
    }
    let bytes: Vec<u8> = pac_url
        .encode_utf16()
        .chain(Some(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    let pac_value = json!({"kind": 1, "bytes": bytes});
    let mut disabled = baseline.clone();
    disabled["raw"] = json!({"kind": 4, "bytes": 0u32.to_le_bytes()});
    disabled["pac"] = pac_value.clone();
    journal.check(&mut system, &pac)?;
    journal.check(&mut system, &flag)?;
    let mut after_url = system.read(&flag)?.ok_or_else(failure)?;
    after_url["pac"] = pac_value.clone();
    journal.prepare_context(&mut system, &flag, Some(after_url))?;
    journal.apply(&mut system, pac, Some(pac_value))?;
    journal.apply(&mut system, flag, Some(disabled))?;
    configured.push("Windows Internet Settings (WinINet)".into());
    Ok(())
}

pub(super) fn notify_windows_inet_refresh() -> Result<(), String> {
    #[link(name = "wininet")]
    unsafe extern "system" {
        fn InternetSetOptionW(
            h: *mut core::ffi::c_void,
            option: u32,
            buffer: *mut core::ffi::c_void,
            len: u32,
        ) -> i32;
    }
    unsafe {
        for option in [39, 37] {
            if InternetSetOptionW(std::ptr::null_mut(), option, std::ptr::null_mut(), 0) == 0 {
                return Err("WinINet не подтвердил обновление настроек; журнал сохранён".into());
            }
        }
    }
    Ok(())
}
