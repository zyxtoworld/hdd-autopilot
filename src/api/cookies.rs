use crate::model::SessionCookie;

pub(super) fn cookie_header_value(cookies: &[SessionCookie]) -> String {
    cookies
        .iter()
        .filter_map(|cookie| {
            let name = cookie.name.trim();
            if name.is_empty() || cookie.value.is_empty() {
                return None;
            }
            Some(format!("{}={}", name, cookie.value))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn merge_session_cookies(
    existing: &[SessionCookie],
    set_cookie_headers: &[String],
) -> Vec<SessionCookie> {
    let mut result = normalize_session_cookies(existing.to_vec());
    for header in set_cookie_headers {
        match parse_set_cookie_action(header) {
            Some(SetCookieAction::Upsert(cookie)) => {
                upsert_cookie(&mut result, cookie);
            }
            Some(SetCookieAction::Remove { name, domain, path }) => {
                result.retain(|cookie| !cookie_matches(cookie, &name, &domain, &path));
            }
            None => {}
        }
    }
    normalize_session_cookies(result)
}

fn upsert_cookie(cookies: &mut Vec<SessionCookie>, cookie: SessionCookie) {
    if let Some(index) = cookies
        .iter()
        .position(|current| cookie_matches(current, &cookie.name, &cookie.domain, &cookie.path))
    {
        cookies[index] = cookie;
    } else {
        cookies.push(cookie);
    }
}

fn cookie_matches(cookie: &SessionCookie, name: &str, domain: &str, path: &str) -> bool {
    cookie.name.trim().eq_ignore_ascii_case(name.trim())
        && cookie.domain.trim().eq_ignore_ascii_case(domain.trim())
        && cookie.path.trim() == path.trim()
}

enum SetCookieAction {
    Upsert(SessionCookie),
    Remove {
        name: String,
        domain: String,
        path: String,
    },
}

fn parse_set_cookie_action(header: &str) -> Option<SetCookieAction> {
    let mut parts = header.split(';');
    let (name, value) = parse_cookie_pair(parts.next()?)?;
    let mut domain = String::new();
    let mut path = String::new();
    let mut expires_at = String::new();
    let mut secure = false;
    let mut http_only = false;

    for part in parts {
        let item = part.trim();
        if item.is_empty() {
            continue;
        }
        if item.eq_ignore_ascii_case("secure") {
            secure = true;
            continue;
        }
        if item.eq_ignore_ascii_case("httponly") {
            http_only = true;
            continue;
        }
        let mut split = item.splitn(2, '=');
        let key = split.next().unwrap_or("").trim();
        let value = split.next().unwrap_or("").trim();
        if key.eq_ignore_ascii_case("domain") {
            domain = value.to_string();
        } else if key.eq_ignore_ascii_case("path") {
            path = value.to_string();
        } else if key.eq_ignore_ascii_case("expires") {
            expires_at = value.to_string();
        }
    }

    if value.is_empty() {
        return Some(SetCookieAction::Remove { name, domain, path });
    }

    Some(SetCookieAction::Upsert(SessionCookie {
        name,
        value,
        domain,
        path,
        expires_at,
        secure,
        http_only,
    }))
}

fn parse_cookie_pair(part: &str) -> Option<(String, String)> {
    let mut split = part.splitn(2, '=');
    let name = split.next()?.trim().to_string();
    let value = split.next().unwrap_or("").trim().to_string();
    if name.is_empty() {
        return None;
    }
    Some((name, value))
}

pub(super) fn normalize_session_cookies(cookies: Vec<SessionCookie>) -> Vec<SessionCookie> {
    let mut result = Vec::with_capacity(cookies.len());
    for mut cookie in cookies {
        cookie.name = cookie.name.trim().to_string();
        cookie.value = cookie.value.trim().to_string();
        cookie.domain = cookie.domain.trim().to_string();
        cookie.path = cookie.path.trim().to_string();
        cookie.expires_at = cookie.expires_at.trim().to_string();
        if cookie.name.is_empty() || cookie.value.is_empty() {
            continue;
        }
        upsert_cookie(&mut result, cookie);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_set_cookie_header() {
        let action = parse_set_cookie_action(
            "session=cookie-123; Path=/; Domain=sub.hdd.sb; HttpOnly; Secure; Expires=Wed, 09 Jun 2027 10:18:14 GMT",
        );
        let Some(SetCookieAction::Upsert(cookie)) = action else {
            panic!("expected upsert cookie");
        };
        assert_eq!(cookie.name, "session");
        assert_eq!(cookie.value, "cookie-123");
        assert_eq!(cookie.domain, "sub.hdd.sb");
        assert_eq!(cookie.path, "/");
        assert_eq!(cookie.expires_at, "Wed, 09 Jun 2027 10:18:14 GMT");
        assert!(cookie.http_only);
        assert!(cookie.secure);
    }

    #[test]
    fn merges_and_removes_session_cookies() {
        let existing = vec![SessionCookie {
            name: "session".to_string(),
            value: "old".to_string(),
            domain: "sub.hdd.sb".to_string(),
            path: "/".to_string(),
            ..SessionCookie::default()
        }];
        let updated = merge_session_cookies(
            &existing,
            &[
                "session=new; Path=/; Domain=sub.hdd.sb".to_string(),
                "refresh=token-1; Path=/; Domain=sub.hdd.sb; HttpOnly".to_string(),
            ],
        );
        assert_eq!(updated.len(), 2);
        assert_eq!(updated[0].value, "new");
        assert_eq!(updated[1].name, "refresh");

        let removed = merge_session_cookies(
            &updated,
            &["session=; Path=/; Domain=sub.hdd.sb; Max-Age=0".to_string()],
        );
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].name, "refresh");
    }

    #[test]
    fn builds_cookie_header() {
        let header = cookie_header_value(&[
            SessionCookie {
                name: "session".to_string(),
                value: "cookie-123".to_string(),
                ..SessionCookie::default()
            },
            SessionCookie {
                name: "refresh".to_string(),
                value: "token-1".to_string(),
                ..SessionCookie::default()
            },
        ]);
        assert_eq!(header, "session=cookie-123; refresh=token-1");
    }
}
