//! `file://` URIs for desktop integration (freedesktop FileManager1).

/// Percent-encodes an absolute POSIX path into a `file://` URI (RFC 8089).
pub fn file_uri(path: &str) -> String {
    let mut uri = String::with_capacity(path.len() + 7);
    uri.push_str("file://");
    for byte in path.bytes() {
        let unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/');
        if unreserved {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_spaces_and_unicode() {
        assert_eq!(file_uri("/home/a b/ü.txt"), "file:///home/a%20b/%C3%BC.txt");
        assert_eq!(file_uri("/tmp/x,y#z"), "file:///tmp/x%2Cy%23z");
    }
}
