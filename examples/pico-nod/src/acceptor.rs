#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptorError {
    MethodNotAllowed,
    PathNotAllowed,
    HeaderTooLarge,
    MissingHeaderEnd,
    MissingContentLength,
    InvalidContentLength,
    BodyTooLarge,
    BodyTruncated,
    ChunkedUnsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicIntentRequest<'a> {
    pub body: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpTlsAcceptor {
    max_header_bytes: usize,
    max_body_bytes: usize,
}

impl HttpTlsAcceptor {
    pub const fn new(max_header_bytes: usize, max_body_bytes: usize) -> Self {
        Self {
            max_header_bytes,
            max_body_bytes,
        }
    }

    pub fn parse<'a>(&self, request: &'a [u8]) -> Result<PublicIntentRequest<'a>, AcceptorError> {
        let header_end = find_header_end(request).ok_or(AcceptorError::MissingHeaderEnd)?;
        if header_end > self.max_header_bytes {
            return Err(AcceptorError::HeaderTooLarge);
        }
        let headers = request.split_at(header_end).0;
        let body = request.split_at(header_end + 4).1;
        let first_line_end = find_line_end(headers).ok_or(AcceptorError::MissingHeaderEnd)?;
        let first_line = headers.split_at(first_line_end).0;
        if !first_line.starts_with(b"POST ") {
            return Err(AcceptorError::MethodNotAllowed);
        }
        if !first_line.starts_with(b"POST /intent ") {
            return Err(AcceptorError::PathNotAllowed);
        }
        if contains_header_value(headers, b"transfer-encoding:", b"chunked") {
            return Err(AcceptorError::ChunkedUnsupported);
        }
        let content_length =
            content_length(headers).ok_or(AcceptorError::MissingContentLength)??;
        if content_length > self.max_body_bytes {
            return Err(AcceptorError::BodyTooLarge);
        }
        if body.len() < content_length {
            return Err(AcceptorError::BodyTruncated);
        }
        Ok(PublicIntentRequest {
            body: body.split_at(content_length).0,
        })
    }

    pub const fn cannot_hold_credentials(&self) -> bool {
        true
    }

    pub const fn cannot_select_routes(&self) -> bool {
        true
    }

    pub const fn cannot_commit_external_actions(&self) -> bool {
        true
    }
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn find_line_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(2).position(|window| window == b"\r\n")
}

fn content_length(headers: &[u8]) -> Option<Result<usize, AcceptorError>> {
    let mut start = 0usize;
    while start < headers.len() {
        let remaining = headers.split_at(start).1;
        let line_len = match find_line_end(remaining) {
            Some(line_len) => line_len,
            None => remaining.len(),
        };
        let line = remaining.split_at(line_len).0;
        if header_name_eq(line, b"content-length:") {
            return Some(parse_decimal(trim_ascii(
                line.split_at(b"content-length:".len()).1,
            )));
        }
        start = start.saturating_add(line_len).saturating_add(2);
    }
    None
}

fn parse_decimal(bytes: &[u8]) -> Result<usize, AcceptorError> {
    if bytes.is_empty() {
        return Err(AcceptorError::InvalidContentLength);
    }
    let mut value = 0usize;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return Err(AcceptorError::InvalidContentLength);
        }
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(usize::from(byte - b'0')))
            .ok_or(AcceptorError::InvalidContentLength)?;
    }
    Ok(value)
}

fn contains_header_value(headers: &[u8], name: &[u8], value: &[u8]) -> bool {
    let mut start = 0usize;
    while start < headers.len() {
        let remaining = headers.split_at(start).1;
        let line_len = match find_line_end(remaining) {
            Some(line_len) => line_len,
            None => remaining.len(),
        };
        let line = remaining.split_at(line_len).0;
        if header_name_eq(line, name)
            && ascii_contains(trim_ascii(line.split_at(name.len()).1), value)
        {
            return true;
        }
        start = start.saturating_add(line_len).saturating_add(2);
    }
    false
}

fn header_name_eq(line: &[u8], name: &[u8]) -> bool {
    line.len() >= name.len() && ascii_eq_ignore_case(line.split_at(name.len()).0, name)
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0usize;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    bytes.split_at(start).1.split_at(end - start).0
}

fn ascii_contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| ascii_eq_ignore_case(window, needle))
}

fn ascii_eq_ignore_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}
