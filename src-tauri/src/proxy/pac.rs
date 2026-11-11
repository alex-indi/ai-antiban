use crate::auditor::rules::ProviderMatrix;

pub const DEFAULT_AI_DOMAINS: &[&str] = &[
    // Anthropic / Claude / Cursor / OpenRouter / Perplexity
    "claude.ai",
    "claude.com",
    "anthropic.com",
    "claudeusercontent.com",
    "console.anthropic.com",
    "statsig.anthropic.com",
    "cursor.sh",
    "api2.cursor.sh",
    "api3.cursor.sh",
    "cursor.com",
    "cursor-cdn.com",
    "openrouter.ai",
    "poe.com",
    "perplexity.ai",
    "pplx.ai",
    // OpenAI / ChatGPT / Microsoft Copilot / DeepSeek / Mistral / Grok
    "chatgpt.com",
    "openai.com",
    "oaistatic.com",
    "oaiusercontent.com",
    "auth0.openai.com",
    "platform.openai.com",
    "chat.openai.com",
    "copilot.microsoft.com",
    "sydney.bing.com",
    "edgeservices.bing.com",
    "copilot.cloud.microsoft",
    "officecopilot.microsoft.com",
    "githubcopilot.com",
    "api.githubcopilot.com",
    "copilot-proxy.githubusercontent.com",
    "default.exp-tas.com",
    "codeium.com",
    "windsurf.com",
    "codeiumdata.com",
    "supermaven.com",
    "deepseek.com",
    "chat.deepseek.com",
    "api.deepseek.com",
    "grok.com",
    "x.ai",
    "api.x.ai",
    "mistral.ai",
    "chat.mistral.ai",
    // Google Gemini / AI Studio / Cloud Code / Antigravity / Labs
    "gemini.google.com",
    "aistudio.google.com",
    "makersuite.google.com",
    "generativelanguage.googleapis.com",
    "aiplatform.googleapis.com",
    "cloudaicompanion.googleapis.com",
    "alkalimakersuite-pa.clients6.google.com",
    "daily-cloudcode-pa.googleapis.com",
    "cloudcode-pa.googleapis.com",
    "daily-cloudcode-pa.sandbox.googleapis.com",
    "autopush-cloudcode-pa.sandbox.googleapis.com",
    "preprod-daily-cloudcode-pa.sandbox.googleapis.com",
    "aicode.googleapis.com",
    "autopush-aicode.sandbox.googleapis.com",
    "preprod-aicode.sandbox.googleapis.com",
    "alkalimakersuiteapplets.pa.googleapis.com",
    "antigravity.google",
    "antigravity.google.com",
    "share.gemini.google",
    "notebooklm.google",
    "notebooklm.google.com",
    "idx.google.com",
    "opal.google.com",
    "labs.google",
];

/// Collects all configured AI domains from the matrix.
pub fn get_all_ai_domains(matrix: &ProviderMatrix) -> Vec<String> {
    let mut domains = Vec::new();
    for provider in matrix.providers.values() {
        for domain in &provider.domains {
            if !domains.contains(domain) {
                domains.push(domain.clone());
            }
        }
    }

    if domains.is_empty() {
        DEFAULT_AI_DOMAINS.iter().map(|s| s.to_string()).collect()
    } else {
        domains
    }
}

/// Collects all configured AI domains from the matrix merged with user-defined custom domains.
pub fn get_all_ai_domains_with_custom(matrix: &ProviderMatrix, custom_domains: &[String]) -> Vec<String> {
    let mut domains = get_all_ai_domains(matrix);
    for cd in custom_domains {
        let clean = cd.trim().trim_start_matches("*.").trim_start_matches('.').to_lowercase();
        if !clean.is_empty() && !domains.iter().any(|d| d.eq_ignore_ascii_case(&clean)) {
            domains.push(clean);
        }
    }
    domains
}

/// Generates a standard Proxy Auto-Configuration (PAC) script routing AI domains to proxy.
/// If `chained_proxy` is specified, non-AI traffic falls back to `PROXY chained_proxy; DIRECT`.
/// Otherwise, non-AI traffic falls back directly to `DIRECT`.
pub fn generate_pac_script(
    domains: &[String],
    proxy_host_port: &str,
    chained_proxy: Option<&str>,
) -> String {
    let domains_json = serde_json::to_string_pretty(domains).unwrap_or_else(|_| "[]".to_string());
    let fallback = match chained_proxy.map(str::trim).filter(|s| !s.is_empty()) {
        Some(cp) => format!("PROXY {cp}; DIRECT"),
        None => "DIRECT".to_string(),
    };

    format!(
        r#"// Zastava Selective PAC Script
// Automatically routes known AI domains through local hold proxy, keeping everything else chained or DIRECT.
function FindProxyForURL(url, host) {{
    var aiDomains = {domains_json};
    host = (host || "").toLowerCase();

    for (var i = 0; i < aiDomains.length; i++) {{
        var d = aiDomains[i];
        if (host === d || shExpMatch(host, "*." + d)) {{
            return "PROXY {proxy_host_port}";
        }}
    }}
    return "{fallback}";
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pac_file_generation() {
        let domains = vec![
            "claude.ai".to_string(),
            "chatgpt.com".to_string(),
            "gemini.google.com".to_string(),
        ];
        let pac = generate_pac_script(&domains, "127.0.0.1:8899", None);

        assert!(pac.contains("function FindProxyForURL(url, host)"));
        assert!(pac.contains("\"claude.ai\""));
        assert!(pac.contains("\"chatgpt.com\""));
        assert!(pac.contains("\"gemini.google.com\""));
        assert!(pac.contains("return \"PROXY 127.0.0.1:8899\";"));
        assert!(pac.contains("return \"DIRECT\";"));
        assert!(pac.contains("host = (host || \"\").toLowerCase();"));
    }

    #[test]
    fn test_pac_file_generation_with_chained_proxy() {
        let domains = vec![
            "claude.ai".to_string(),
            "chatgpt.com".to_string(),
        ];
        let pac = generate_pac_script(&domains, "127.0.0.1:8899", Some("127.0.0.1:10808"));

        assert!(pac.contains("function FindProxyForURL(url, host)"));
        assert!(pac.contains("return \"PROXY 127.0.0.1:8899\";"));
        assert!(pac.contains("return \"PROXY 127.0.0.1:10808; DIRECT\";"));
    }

    #[test]
    fn test_pac_with_custom_domains() {
        let matrix = ProviderMatrix::load_bundled().unwrap();
        let custom = vec!["custom-ai.company.org".to_string(), "claude.ai".to_string()];
        let merged = get_all_ai_domains_with_custom(&matrix, &custom);
        assert!(merged.contains(&"custom-ai.company.org".to_string()));
        assert!(merged.contains(&"claude.ai".to_string()));
    }
}
