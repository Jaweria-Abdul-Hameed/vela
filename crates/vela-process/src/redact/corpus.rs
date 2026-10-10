//! The redaction corpus: one case per class of secret named by `OBSERVABILITY.md` ("tokens, authorization headers,
//! credential-like environment values, cookies, known secret patterns"), each in the forms it takes in real output.
//!
//! The same corpus is run through every ingestion point: plain text (`pattern`), chunked process output (`stream`),
//! the JSON log pipeline (`log`) and serialized journal payloads (`log`, as JSON documents).
//!
//! Token-shaped values are assembled from pieces so that no secret scanner mistakes this file for a leak.

/// One corpus entry.
pub(super) struct Case {
    /// What the case covers.
    pub(super) name: &'static str,
    /// The text given to the redactor.
    pub(super) text: String,
    /// Substrings that must not appear in the output.
    pub(super) secrets: Vec<String>,
    /// Substrings that must still appear in the output.
    pub(super) kept: Vec<&'static str>,
}

fn case(name: &'static str, text: String, secrets: &[&str], kept: &[&'static str]) -> Case {
    Case {
        name,
        text,
        secrets: secrets.iter().map(|secret| (*secret).to_owned()).collect(),
        kept: kept.to_vec(),
    }
}

/// A token-shaped value: `prefix` followed by `body`, never written contiguously in this file.
fn shaped(prefix: &str, body: &str) -> String {
    [prefix, body].concat()
}

pub(super) fn corpus() -> Vec<Case> {
    let github = shaped("ghp", "_0123456789abcdefghijABCDEFGHIJ012345");
    let github_fine = shaped(
        "github_pat",
        "_11ABCDEFG0abcdefghijkl_mnopqrstuvwxyz0123456789ABCDEFGH",
    );
    let gitlab = shaped("glpat", "-abcdefghij0123456789");
    let google_key = shaped("AIza", "SyA0123456789abcdefghijklmnopqrstuv_-");
    let google_access = shaped("ya29", ".a0AbCdEfGhIjKlMnOpQrStUvWxYz-0123456789_abc");
    let google_refresh = shaped("1/", "/0abcdefghijklmnopqrstuvwxyzABCDEFGHIJ");
    let aws_id = shaped("AKIA", "IOSFODNN7EXAMPLE");
    let aws_secret = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
    let slack = shaped("xoxb", "-123456789012-abcdefghijklmnop");
    let openai = shaped("sk", "-proj-abcdefghijklmnopqrstuvwxyz012345");
    let stripe = shaped("sk_live", "_abcdefghijklmnop0123456789");
    let npm = shaped("npm", "_abcdefghijklmnopqrstuvwxyz0123456789");
    let jwt = [
        shaped("eyJ", "hbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"),
        shaped("eyJ", "zdWIiOiIxMjM0NTY3ODkwIn0"),
        "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c".to_owned(),
    ]
    .join(".");
    let private_key = [
        shaped("-----BEGIN RSA ", "PRIVATE KEY-----"),
        "MIIEowIBAAKCAQEAu1SU1LfVLPHCozMxH2Mo".to_owned(),
        "4lgOEePzNm0tRgeLezV6ffAt0gunVTLw7onL".to_owned(),
        shaped("-----END RSA ", "PRIVATE KEY-----"),
    ]
    .join("\n");

    vec![
        // Tokens.
        case(
            "bearer token in prose",
            "request failed with Bearer abcdefghijklmnopqrstuvwxyz012345 (401)".to_owned(),
            &["abcdefghijklmnopqrstuvwxyz012345"],
            &["request failed with Bearer ", "(401)"],
        ),
        case(
            "github token in a command line",
            format!("gh api repos/o/r --token {github} --paginate"),
            &[&github],
            &["gh api", "repos/o/r"],
        ),
        case(
            "github fine-grained token alone on a line",
            format!("warning: found {github_fine}."),
            &[&github_fine],
            &["warning: found "],
        ),
        case("gitlab token", format!("using {gitlab} for the mirror"), &[&gitlab], &["using ", " for the mirror"]),
        case(
            "google api key in a url",
            format!("GET https://generativelanguage.googleapis.com/v1/models?key={google_key}&alt=json"),
            &[&google_key],
            &["https://generativelanguage.googleapis.com/v1/models"],
        ),
        case(
            "google oauth access and refresh tokens",
            format!("access {google_access} refresh {google_refresh} done"),
            &[&google_access, &google_refresh],
            &["access ", " refresh ", " done"],
        ),
        case("aws access key id", format!("aws: key {aws_id} rejected"), &[&aws_id], &["aws: key ", " rejected"]),
        case("slack token", format!("slack {slack}"), &[&slack], &["slack "]),
        case("sk- api key", format!("provider key {openai} loaded"), &[&openai], &["provider key ", " loaded"]),
        case("stripe key", format!("stripe {stripe}"), &[&stripe], &["stripe "]),
        case("npm token", format!("//registry.npmjs.org/:_authToken={npm}"), &[&npm], &["//registry.npmjs.org/"]),
        case("json web token", format!("session {jwt} expired"), &[&jwt, "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"], &["session ", " expired"]),
        // Authorization headers.
        case(
            "authorization header, basic",
            "> GET / HTTP/1.1\n> Authorization: Basic YWRhOmh1bnRlcjI=\n> Accept: */*".to_owned(),
            &["YWRhOmh1bnRlcjI="],
            &["> GET / HTTP/1.1", "> Authorization: ", "> Accept: */*"],
        ),
        case(
            "authorization header, bearer, in a curl command",
            "curl -H \"Authorization: Bearer s3cr3t-t0ken-value\" https://api.example.com".to_owned(),
            &["s3cr3t-t0ken-value"],
            &["curl -H \"Authorization: "],
        ),
        case(
            "proxy authorization and api key headers",
            "Proxy-Authorization: Digest username=\"ada\", response=\"6629fae49393a05397450978507c4ef1\"\nX-Api-Key: k-9f8e7d6c5b4a"
                .to_owned(),
            &["6629fae49393a05397450978507c4ef1", "k-9f8e7d6c5b4a"],
            &["Proxy-Authorization: ", "X-Api-Key: "],
        ),
        case(
            "git extra header",
            "git -c http.extraheader=\"AUTHORIZATION: basic eDp0b2tlbg==\" fetch".to_owned(),
            &["eDp0b2tlbg=="],
            &["git -c http.extraheader="],
        ),
        // Credential-like environment values.
        case(
            "environment dump",
            format!("PATH=/usr/bin\nDB_PASSWORD=hunter2\nAWS_SECRET_ACCESS_KEY={aws_secret}\nHOME=/home/ada"),
            &["hunter2", aws_secret],
            &["PATH=/usr/bin", "DB_PASSWORD=", "AWS_SECRET_ACCESS_KEY=", "HOME=/home/ada"],
        ),
        case(
            "environment assignments in shells",
            "export API_KEY=\"k 1 2 3\"; $env:CLIENT_SECRET = 'p@ss w0rd'; set SMTP_PASS=letmein".to_owned(),
            &["k 1 2 3", "p@ss w0rd", "letmein"],
            &["export API_KEY=", "$env:CLIENT_SECRET = ", "set SMTP_PASS="],
        ),
        case(
            "command-line flags",
            "mysql --user root --password tr0ub4dor --host db && tool --api-key=abc123 run".to_owned(),
            &["tr0ub4dor", "abc123"],
            &["mysql --user root --password ", " --host db", "tool --api-key=", " run"],
        ),
        case(
            "configuration file lines",
            "password: correct horse battery staple\nclientSecret = \"s3cr3t\"\n  auth_token: 'abc def'".to_owned(),
            &["correct horse battery staple", "s3cr3t", "abc def"],
            &["password: ", "clientSecret = ", "auth_token: "],
        ),
        // Cookies.
        case(
            "cookie request header",
            "Cookie: sid=31d4d96e407aad42; theme=\"dark mode\"; csrftoken=abc\nHost: example.com".to_owned(),
            &["31d4d96e407aad42", "dark mode", "csrftoken=abc"],
            &["Cookie: ", "Host: example.com"],
        ),
        case(
            "set-cookie response header",
            "< Set-Cookie: session=eyJhIjoxfQ.sig; Path=/; HttpOnly\n< Content-Type: text/html".to_owned(),
            &["eyJhIjoxfQ.sig"],
            &["< Set-Cookie: ", "< Content-Type: text/html"],
        ),
        // Known secret patterns.
        case(
            "credentials in a url",
            "remote: https://ada:p%40ssw0rd@github.com/o/r.git (fetch)".to_owned(),
            &["ada:p%40ssw0rd", "p%40ssw0rd"],
            &["remote: https://", "@github.com/o/r.git (fetch)"],
        ),
        case(
            "token as the user of a url",
            format!("fatal: unable to access 'https://{github}@github.com/o/r.git/'"),
            &[&github],
            &["fatal: unable to access 'https://", "@github.com/o/r.git/'"],
        ),
        case(
            "database url",
            "DATABASE_URL=postgres://app:s3cr3tpw@db.internal:5432/app".to_owned(),
            &["s3cr3tpw"],
            &["DATABASE_URL=postgres://", "@db.internal:5432/app"],
        ),
        case(
            "private key block",
            format!("key follows\n{private_key}\nend of key"),
            &["MIIEowIBAAKCAQEAu1SU1LfVLPHCozMxH2Mo", "4lgOEePzNm0tRgeLezV6ffAt0gunVTLw7onL"],
            &["key follows", "end of key"],
        ),
        // The same secrets inside JSON, as journal payloads and stream-json output carry them.
        case(
            "json members",
            format!(
                "{{\"kind\":\"CommandStarted\",\"env\":{{\"GITHUB_TOKEN\":\"{github}\",\"CI\":\"true\"}},\"password\":\"hunter2\"}}"
            ),
            &[&github, "hunter2"],
            &["\"kind\":\"CommandStarted\"", "\"CI\":\"true\""],
        ),
        case(
            "json string holding a command line",
            "{\"command\":\"curl -H \\\"Authorization: Bearer s3cr3t-t0ken-value\\\" https://x\",\"exit\":0}"
                .to_owned(),
            &["s3cr3t-t0ken-value"],
            &["\"exit\":0"],
        ),
        case(
            "json name and value pair",
            "{\"headers\":[{\"name\":\"Cookie\",\"value\":\"sid=31d4d96e407aad42\"},{\"name\":\"Accept\",\"value\":\"*/*\"}]}"
                .to_owned(),
            &["31d4d96e407aad42"],
            &["\"name\":\"Accept\",\"value\":\"*/*\""],
        ),
        case(
            "truncated json",
            "{\"type\":\"tool_result\",\"api_key\":\"k-9f8e7d6c5b4a\",\"text\":\"DB_PASSWORD=hunter2".to_owned(),
            &["k-9f8e7d6c5b4a", "hunter2"],
            &["\"type\":\"tool_result\""],
        ),
    ]
}
