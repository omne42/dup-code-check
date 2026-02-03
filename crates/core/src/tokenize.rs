#[derive(Debug, Clone)]
pub(crate) struct BlockNode {
    pub(crate) start_token: usize,
    pub(crate) end_token: usize,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) depth: u32,
    pub(crate) children: Vec<usize>,
}

#[derive(Debug)]
pub(crate) struct TokenizedText {
    pub(crate) tokens: Vec<u32>,
    pub(crate) token_lines: Vec<u32>,
}

pub(crate) fn tokenize_for_dup_detection(text: &str) -> TokenizedText {
    const TOK_IDENT: u32 = 1;
    const TOK_NUM: u32 = 2;
    const TOK_STR: u32 = 3;
    const TOK_PUNCT_BASE: u32 = 10_000;

    fn keyword_token(ident: &str) -> Option<u32> {
        Some(match ident {
            "if" => 100,
            "else" => 101,
            "for" => 102,
            "while" => 103,
            "do" => 104,
            "switch" => 105,
            "case" => 106,
            "break" => 107,
            "continue" => 108,
            "return" => 109,
            "try" => 110,
            "catch" => 111,
            "finally" => 112,
            "throw" => 113,
            "fn" => 114,
            "function" => 115,
            "class" => 116,
            "struct" => 117,
            "enum" => 118,
            "impl" => 119,
            "trait" => 120,
            "const" => 121,
            "let" => 122,
            "var" => 123,
            "static" => 124,
            "public" => 125,
            "private" => 126,
            "protected" => 127,
            "async" => 128,
            "await" => 129,
            _ => return None,
        })
    }

    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut line: u32 = 1;
    let mut at_line_start = true;

    let mut tokens = Vec::new();
    let mut token_lines = Vec::new();

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            line = line.saturating_add(1);
            i += 1;
            at_line_start = true;
            continue;
        }
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        let was_at_line_start = at_line_start;
        at_line_start = false;

        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() {
                if bytes[i] == b'\n' {
                    line = line.saturating_add(1);
                    at_line_start = true;
                }
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if b == b'#' && was_at_line_start {
            i += 1;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if b == b'"' || b == b'\'' {
            let quote = b;
            let start_line = line;
            i += 1;
            while i < bytes.len() {
                let c = bytes[i];
                if c == b'\n' {
                    line = line.saturating_add(1);
                }
                if c == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if c == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            tokens.push(TOK_STR);
            token_lines.push(start_line);
            continue;
        }

        if (b as char).is_ascii_alphabetic() || b == b'_' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let c = bytes[i];
                if (c as char).is_ascii_alphanumeric() || c == b'_' {
                    i += 1;
                } else {
                    break;
                }
            }
            let ident = &text[start..i];
            let tok = keyword_token(ident).unwrap_or(TOK_IDENT);
            tokens.push(tok);
            token_lines.push(line);
            continue;
        }

        if (b as char).is_ascii_digit() {
            i += 1;
            while i < bytes.len() && ((bytes[i] as char).is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            tokens.push(TOK_NUM);
            token_lines.push(line);
            continue;
        }

        tokens.push(TOK_PUNCT_BASE + u32::from(b));
        token_lines.push(line);
        i += 1;
    }

    TokenizedText {
        tokens,
        token_lines,
    }
}

pub(crate) fn parse_brace_blocks(tokens: &[u32], token_lines: &[u32]) -> Vec<BlockNode> {
    const TOK_PUNCT_BASE: u32 = 10_000;
    let open = TOK_PUNCT_BASE + u32::from(b'{');
    let close = TOK_PUNCT_BASE + u32::from(b'}');

    let mut nodes: Vec<BlockNode> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    for (idx, &tok) in tokens.iter().enumerate() {
        if tok == open {
            let depth = (stack.len() as u32) + 1;
            let node_id = nodes.len();
            nodes.push(BlockNode {
                start_token: idx,
                end_token: idx,
                start_line: token_lines.get(idx).copied().unwrap_or(1),
                end_line: token_lines.get(idx).copied().unwrap_or(1),
                depth,
                children: Vec::new(),
            });
            if let Some(parent_id) = stack.last().copied() {
                nodes[parent_id].children.push(node_id);
            }
            stack.push(node_id);
        } else if tok == close {
            let Some(node_id) = stack.pop() else {
                continue;
            };
            nodes[node_id].end_token = idx;
            nodes[node_id].end_line = token_lines
                .get(idx)
                .copied()
                .unwrap_or(nodes[node_id].start_line);
        }
    }

    nodes
}
