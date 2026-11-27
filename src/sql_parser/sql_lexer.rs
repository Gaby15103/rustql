use crate::sql_parser::sql_token::{Keyword, Token};

#[derive(Debug, PartialEq)]
pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        Some(ch)
    }
    fn skip_whitespace(&mut self) {
        loop {
            while let Some(ch) = self.peek() {
                if ch.is_whitespace() {
                    self.advance();
                } else {
                    break;
                }
            }

            // Skip `--` single-line SQL comments
            if self.peek() == Some('-') && self.peek_next() == Some('-') {
                while let Some(ch) = self.advance() {
                    if ch == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace();
            let Some(ch) = self.peek() else {
                tokens.push(Token::Eof);
                break;
            };

            if ch.is_ascii_alphabetic() || ch == '_' {
                tokens.push(self.read_word());
            } else if ch.is_ascii_digit() {
                tokens.push(self.read_number()?);
            } else if ch == '\'' {
                tokens.push(self.read_string()?);
            } else {
                tokens.push(self.read_symbol()?);
            }
        }
        Ok(tokens)
    }

    fn read_word(&mut self) -> Token {
        let mut word = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                word.push(self.advance().unwrap());
            } else {
                break;
            }
        }

        match word.to_ascii_uppercase().as_str() {
            "SELECT" => Token::Keyword(Keyword::Select),
            "FROM" => Token::Keyword(Keyword::From),
            "WHERE" => Token::Keyword(Keyword::Where),
            "LIMIT" => Token::Keyword(Keyword::Limit),
            "INSERT" => Token::Keyword(Keyword::Insert),
            "INTO" => Token::Keyword(Keyword::Into),
            "VALUES" => Token::Keyword(Keyword::Values),
            "CREATE" => Token::Keyword(Keyword::Create),
            "TABLE" => Token::Keyword(Keyword::Table),
            "IF" => Token::Keyword(Keyword::If),
            "EXISTS" => Token::Keyword(Keyword::Exists),
            "PRIMARY" => Token::Keyword(Keyword::Primary),
            "KEY" => Token::Keyword(Keyword::Key),
            "INT" | "INTEGER" => Token::Keyword(Keyword::Int),
            "TEXT" | "VARCHAR" => Token::Keyword(Keyword::Text),
            "BOOL" | "BOOLEAN" => Token::Keyword(Keyword::Bool),
            "UPDATE" => Token::Keyword(Keyword::Update),
            "SET" => Token::Keyword(Keyword::Set),
            "DELETE" => Token::Keyword(Keyword::Delete),
            "DROP" => Token::Keyword(Keyword::Drop),
            "AS" => Token::Keyword(Keyword::As),
            "AND" => Token::Keyword(Keyword::And),
            "OR" => Token::Keyword(Keyword::Or),
            "NOT" => Token::Keyword(Keyword::Not),
            "TRUE" => Token::Keyword(Keyword::True),
            "FALSE" => Token::Keyword(Keyword::False),
            "NULL" => Token::Keyword(Keyword::Null),
            "ORDER" => Token::Keyword(Keyword::Order),
            "BY" => Token::Keyword(Keyword::By),
            "ASC" => Token::Keyword(Keyword::Asc),
            "DESC" => Token::Keyword(Keyword::Desc),
            "UNIQUE" => Token::Keyword(Keyword::Unique),
            _ => Token::Identifier(word),
        }
    }

    fn read_number(&mut self) -> Result<Token, String> {
        let mut num_str = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                num_str.push(self.advance().unwrap());
            } else {
                break;
            }
        }
        let val = num_str
            .parse::<i64>()
            .map_err(|e| format!("Failed to parse number: {}", e))?;
        Ok(Token::Number(val))
    }
    fn read_string(&mut self) -> Result<Token, String> {
        self.advance();
        let mut value = String::new();

        while let Some(ch) = self.advance() {
            if ch == '\'' {
                if self.peek() == Some('\'') {
                    self.advance();
                    value.push('\'');
                } else {
                    return Ok(Token::StringLiteral(value));
                }
            } else {
                value.push(ch);
            }
        }

        Err("Unterminated string literal".to_string())
    }

    fn read_symbol(&mut self) -> Result<Token, String> {
        let ch = self.advance().unwrap();
        let tok = match ch {
            '=' => Token::Eq,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            ',' => Token::Comma,
            ';' => Token::Semicolon,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Token::NotEq
                } else {
                    return Err("Unexpected '!' (did you mean '!='?)".to_string());
                }
            }
            '<' => match self.peek() {
                Some('=') => {
                    self.advance();
                    Token::LtEq
                }
                Some('>') => {
                    self.advance();
                    Token::NotEq
                }
                _ => Token::Lt,
            },
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Token::GtEq
                } else {
                    Token::Gt
                }
            }
            _ => return Err(format!("Unexpected character '{}'", ch)),
        };
        Ok(tok)
    }
}
