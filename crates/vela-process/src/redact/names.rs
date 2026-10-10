//! Which names (environment variables, headers, JSON members, command-line flags) hold credentials.

/// The longest name that is examined; longer runs are not names.
pub(super) const MAX_NAME_LEN: usize = 64;

/// A segment containing one of these is a credential wherever it appears (`DB_PASSWORD`, `clientSecret`).
const STEMS: [&[u8]; 6] = [
    b"password",
    b"passwd",
    b"passphrase",
    b"secret",
    b"credential",
    b"authorization",
];

/// Whole segments that name a credential.
const SEGMENTS: [&[u8]; 8] = [
    b"pwd", b"cookie", b"cookies", b"jwt", b"bearer", b"creds", b"apikey", b"auth",
];

/// A `key` or `keys` segment is a credential only after one of these (`api_key`, `private-keys`); `sort_key` is
/// not.
const KEY_QUALIFIERS: [&[u8]; 16] = [
    b"api",
    b"access",
    b"private",
    b"signing",
    b"encryption",
    b"master",
    b"ssh",
    b"gpg",
    b"license",
    b"account",
    b"storage",
    b"shared",
    b"client",
    b"service",
    b"deploy",
    b"auth",
];

/// A plural `tokens` segment is a credential list only after one of these (`auth_tokens`); `input_tokens` and
/// `max_tokens` are counters.
const TOKEN_QUALIFIERS: [&[u8]; 11] = [
    b"auth",
    b"access",
    b"api",
    b"refresh",
    b"bearer",
    b"oauth",
    b"personal",
    b"id",
    b"deploy",
    b"service",
    b"bot",
];

/// A name ending in one of these describes a credential rather than holding one (`max_token_count`, `token_url`).
const DESCRIPTIVE_SUFFIXES: [&[u8]; 20] = [
    b"count",
    b"counts",
    b"limit",
    b"usage",
    b"used",
    b"total",
    b"budget",
    b"length",
    b"len",
    b"size",
    b"type",
    b"ttl",
    b"expiry",
    b"expires",
    b"endpoint",
    b"url",
    b"uri",
    b"path",
    b"file",
    b"name",
];

/// Whether a value stored under `name` is a credential.
///
/// Names are compared case-insensitively after splitting on `_`, `-`, `.` and camel-case boundaries, so
/// `AWS_SECRET_ACCESS_KEY`, `x-api-key`, `clientSecret` and `Set-Cookie` all match. Plural counters (`input_tokens`)
/// and descriptive names (`token_url`) do not, while credential lists (`auth_tokens`, `api_keys`) do.
pub fn is_credential_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    // The shell's working directory variables hold paths; they are not the `pwd` of a connection string.
    if bytes.is_empty() || bytes.len() > MAX_NAME_LEN || matches!(name, "PWD" | "OLDPWD") {
        return false;
    }
    // Normalized form: lower case, every separator and camel-case boundary becomes `_`. At most two bytes per input byte.
    let mut normalized = [0u8; MAX_NAME_LEN * 2];
    let mut len = 0;
    for (index, &byte) in bytes.iter().enumerate() {
        if !byte.is_ascii_alphanumeric() {
            normalized[len] = b'_';
            len += 1;
            continue;
        }
        if index > 0 && byte.is_ascii_uppercase() {
            let previous = bytes[index - 1];
            let next_is_lower = bytes.get(index + 1).is_some_and(u8::is_ascii_lowercase);
            let boundary = previous.is_ascii_lowercase()
                || previous.is_ascii_digit()
                || (previous.is_ascii_uppercase() && next_is_lower);
            if boundary {
                normalized[len] = b'_';
                len += 1;
            }
        }
        normalized[len] = byte.to_ascii_lowercase();
        len += 1;
    }

    let segments = || {
        normalized[..len]
            .split(|byte| *byte == b'_')
            .filter(|s| !s.is_empty())
    };
    let count = segments().count();
    let Some(last) = segments().next_back() else {
        return false;
    };
    if count > 1 && DESCRIPTIVE_SUFFIXES.contains(&last) {
        return false;
    }

    let mut previous: &[u8] = b"";
    for (index, segment) in segments().enumerate() {
        let is_last = index + 1 == count;
        let credential = STEMS.iter().any(|stem| contains(segment, stem))
            || segment.ends_with(b"token")
            || (segment == b"tokens" && TOKEN_QUALIFIERS.contains(&previous))
            || segment.ends_with(b"apikey")
            || segment.ends_with(b"accesskey")
            || segment.ends_with(b"privatekey")
            // `auth` alone is too common inside descriptive names (`auth_method`); it counts only as the final word.
            || (segment == b"auth" && is_last)
            || (segment != b"auth" && SEGMENTS.contains(&segment))
            // `pass` alone is a test verdict (`PASS: 12`); `db_pass` is a credential.
            || (segment == b"pass" && count > 1)
            || (matches!(segment, b"key" | b"keys") && KEY_QUALIFIERS.contains(&previous));
        if credential {
            return true;
        }
        previous = segment;
    }
    false
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_names_are_recognized_in_every_naming_style() {
        for name in [
            "password",
            "PASSWORD",
            "DB_PASSWORD",
            "dbPassword",
            "mysql-passwd",
            "ssh.passphrase",
            "pwd",
            "db_pass",
            "secret",
            "CLIENT_SECRET",
            "clientSecret",
            "AWS_SECRET_ACCESS_KEY",
            "token",
            "GITHUB_TOKEN",
            "accessToken",
            "refresh_token",
            "auth_tokens",
            "ACCESS_TOKENS",
            "apiTokens",
            "api_keys",
            "sshKeys",
            "authtoken",
            "x-auth-token",
            "api_key",
            "apiKey",
            "APIKEY",
            "x-api-key",
            "X-Api-Key",
            "private_key",
            "SSH_PRIVATE_KEY",
            "signing-key",
            "Authorization",
            "Proxy-Authorization",
            "HTTP_AUTHORIZATION",
            "Cookie",
            "Set-Cookie",
            "auth",
            "proxy_auth",
            "credentials",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "jwt",
            "--password",
            "HTTPToken",
        ] {
            assert!(
                is_credential_name(name),
                "{name} should be a credential name"
            );
        }
    }

    #[test]
    fn ordinary_names_are_not_credentials() {
        for name in [
            "",
            "user",
            "username",
            "PATH",
            "HOME",
            "key",
            "sort_key",
            "primary_key",
            "keyboard",
            "tokens",
            "input_tokens",
            "max_tokens",
            "total_tokens",
            "prompt_tokens",
            "sort_keys",
            "token_count",
            "max_token_limit",
            "token_url",
            "token_endpoint",
            "password_file",
            "secret_name",
            "cookie_name",
            "tokenizer",
            "auth_method",
            "author",
            "pass",
            "PASS",
            "passed",
            "compass",
            "PWD",
            "OLDPWD",
            "run_id",
            "session_id",
            "conversation_id",
            "a-name-that-is-far-too-long-to-be-a-real-identifier-and-keeps-going-on-and-on",
        ] {
            assert!(
                !is_credential_name(name),
                "{name} should not be a credential name"
            );
        }
    }
}
