use crate::sql_parser::OrderBy;
use crate::sql_parser::sql_token::{
    Assignment, BinaryOperator, ColumnDef, CreateTableStmt, DataType, DeleteStmt, Expr, InsertStmt,
    Keyword, LiteralValue, SelectItem, SelectStmt, Statement, Token, UnaryOperator, UpdateStmt,
};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn peek_nth(&self, offset: usize) -> &Token {
        self.tokens.get(self.pos + offset).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.peek().clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn consume_keyword(&mut self, expected: Keyword) -> bool {
        if matches!(self.peek(), Token::Keyword(kw) if *kw == expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_keyword(&mut self, expected: Keyword) -> Result<(), String> {
        if self.consume_keyword(expected) {
            Ok(())
        } else {
            Err(format!(
                "Expected keyword {:?}, found {:?}",
                expected,
                self.peek()
            ))
        }
    }

    fn consume_token(&mut self, expected: &Token) -> bool {
        if self.peek() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_token(&mut self, expected: Token) -> Result<(), String> {
        if self.consume_token(&expected) {
            Ok(())
        } else {
            Err(format!("expected {:?} but got {:?}", expected, self.peek()))
        }
    }

    fn expect_identifier(&mut self) -> Result<String, String> {
        match self.advance() {
            Token::Identifier(name) => Ok(name),
            other => Err(format!("Expected identifier, found {:?}", other)),
        }
    }

    pub fn parse_statements(&mut self) -> Result<Vec<Statement>, String> {
        let mut stmts = Vec::new();
        while self.peek() != &Token::Eof {
            // Skip stray semicolons
            if self.consume_token(&Token::Semicolon) {
                continue;
            }
            stmts.push(self.parse_statement()?);
        }
        Ok(stmts)
    }

    pub fn parse_statement(&mut self) -> Result<Statement, String> {
        let stmt = match self.peek() {
            Token::Keyword(Keyword::Select) => Statement::Select(self.parse_select()?),
            Token::Keyword(Keyword::Insert) => Statement::Insert(self.parse_insert()?),
            Token::Keyword(Keyword::Create) => Statement::CreateTable(self.parse_create_table()?),
            Token::Keyword(Keyword::Update) => Statement::Update(self.parse_update()?),
            Token::Keyword(Keyword::Delete) => Statement::Delete(self.parse_delete()?),
            other => {
                return Err(format!(
                    "Unexpected token at start of statement: {:?}",
                    other
                ));
            }
        };
        self.consume_token(&Token::Semicolon);
        Ok(stmt)
    }

    fn parse_select(&mut self) -> Result<SelectStmt, String> {
        self.expect_keyword(Keyword::Select)?;

        let mut columns = Vec::new();
        loop {
            if self.consume_token(&Token::Star) {
                columns.push(SelectItem::Wildcard);
            } else {
                let expr = self.parse_expr(0)?;
                if self.consume_keyword(Keyword::As) {
                    let alias = self.expect_identifier()?;
                    columns.push(SelectItem::AliasedExpr { expr, alias });
                } else if let Token::Identifier(_) = self.peek() {
                    let alias = self.expect_identifier()?;
                    columns.push(SelectItem::AliasedExpr { expr, alias });
                } else {
                    columns.push(SelectItem::Expr(expr));
                }
            }
            if !self.consume_token(&Token::Comma) {
                break;
            }
        }

        self.expect_keyword(Keyword::From)?;
        let from = self.expect_identifier()?;

        let where_clause = if self.consume_keyword(Keyword::Where) {
            Some(self.parse_expr(0)?)
        } else {
            None
        };

        let order_by = if self.consume_keyword(Keyword::Order) {
            self.expect_keyword(Keyword::By)?;
            let column = self.expect_identifier()?;

            let descending = if self.consume_keyword(Keyword::Desc) {
                true
            } else {
                self.consume_keyword(Keyword::Asc);
                false
            };

            Some(OrderBy { column, descending })
        } else {
            None
        };

        let limit = if self.consume_keyword(Keyword::Limit) {
            match self.advance() {
                Token::Number(n) if n >= 0 => Some(n as usize),
                other => return Err(format!("Expected non-negative limit, found {:?}", other)),
            }
        } else {
            None
        };

        Ok(SelectStmt {
            columns,
            from,
            where_clause,
            limit,
            order_by,
        })
    }

    fn parse_insert(&mut self) -> Result<InsertStmt, String> {
        self.expect_keyword(Keyword::Insert)?;
        self.expect_keyword(Keyword::Into)?;
        let table = self.expect_identifier()?;

        let columns = if self.consume_token(&Token::LParen) {
            let mut cols = Vec::new();
            loop {
                cols.push(self.expect_identifier()?);
                if !self.consume_token(&Token::Comma) {
                    break;
                }
            }
            self.expect_token(Token::RParen)?;
            Some(cols)
        } else {
            None
        };
        self.expect_keyword(Keyword::Values)?;

        let mut values = Vec::new();
        loop {
            self.expect_token(Token::LParen)?;
            let mut row = Vec::new();
            loop {
                row.push(self.parse_expr(0)?);
                if !self.consume_token(&Token::Comma) {
                    break;
                }
            }
            self.expect_token(Token::RParen)?;
            values.push(row);

            if !self.consume_token(&Token::Comma) {
                break;
            }
        }
        Ok(InsertStmt {
            table,
            columns,
            values,
        })
    }

    fn parse_create_table(&mut self) -> Result<CreateTableStmt, String> {
        self.expect_keyword(Keyword::Create)?;
        self.expect_keyword(Keyword::Table)?;

        let if_not_exists = if self.consume_keyword(Keyword::If) {
            self.expect_keyword(Keyword::Not)?;
            self.expect_keyword(Keyword::Exists)?;
            true
        } else {
            false
        };

        let table = self.expect_identifier()?;
        self.expect_token(Token::LParen)?;

        let mut columns = Vec::new();
        loop {
            let name = self.expect_identifier()?;
            let data_type = match self.advance() {
                Token::Keyword(Keyword::Int) => DataType::Int,
                Token::Keyword(Keyword::Text) => DataType::Text,
                Token::Keyword(Keyword::Bool) => DataType::Bool,
                other => return Err(format!("Expected column data type, found {:?}", other)),
            };

            let mut is_primary_key = false;
            let mut is_nullable = true;

            // Parse optional column constraints in any order
            loop {
                if self.consume_keyword(Keyword::Primary) {
                    self.expect_keyword(Keyword::Key)?;
                    is_primary_key = true;
                    is_nullable = false;
                } else if self.consume_keyword(Keyword::Not) {
                    self.expect_keyword(Keyword::Null)?;
                    is_nullable = false;
                } else if self.consume_keyword(Keyword::Null) {
                    is_nullable = true;
                } else {
                    break;
                }
            }

            columns.push(ColumnDef {
                name,
                data_type,
                is_primary_key,
                is_nullable,
            });

            if !self.consume_token(&Token::Comma) {
                break;
            }
        }

        self.expect_token(Token::RParen)?;

        Ok(CreateTableStmt {
            table,
            if_not_exists,
            columns,
        })
    }

    fn parse_update(&mut self) -> Result<UpdateStmt, String> {
        self.expect_keyword(Keyword::Update)?;
        let table = self.expect_identifier()?;
        self.expect_keyword(Keyword::Set)?;

        let mut assignments = Vec::new();
        loop {
            let column = self.expect_identifier()?;
            self.expect_token(Token::Eq)?;
            let value = self.parse_expr(0)?;
            assignments.push(Assignment { column, value });

            if !self.consume_token(&Token::Comma) {
                break;
            }
        }

        let where_clause = if self.consume_keyword(Keyword::Where) {
            Some(self.parse_expr(0)?)
        } else {
            None
        };

        Ok(UpdateStmt {
            table,
            assignments,
            where_clause,
        })
    }

    fn parse_delete(&mut self) -> Result<DeleteStmt, String> {
        self.expect_keyword(Keyword::Delete)?;
        self.expect_keyword(Keyword::From)?;
        let table = self.expect_identifier()?;

        let where_clause = if self.consume_keyword(Keyword::Where) {
            Some(self.parse_expr(0)?)
        } else {
            None
        };

        Ok(DeleteStmt {
            table,
            where_clause,
        })
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, String> {
        let mut lhs = match self.advance() {
            Token::Number(n) => Expr::Literal(LiteralValue::Integer(n)),
            Token::StringLiteral(s) => Expr::Literal(LiteralValue::String(s)),
            Token::Keyword(Keyword::True) => Expr::Literal(LiteralValue::Boolean(true)),
            Token::Keyword(Keyword::False) => Expr::Literal(LiteralValue::Boolean(false)),
            Token::Keyword(Keyword::Null) => Expr::Literal(LiteralValue::Null),

            Token::Identifier(name) => {
                // Check if this identifier starts a function call: `COUNT(*)`
                if self.consume_token(&Token::LParen) {
                    let mut args = Vec::new();
                    if !self.consume_token(&Token::RParen) {
                        loop {
                            if self.consume_token(&Token::Star) {
                                args.push(Expr::Identifier("*".to_string()));
                            } else {
                                args.push(self.parse_expr(0)?);
                            }
                            if !self.consume_token(&Token::Comma) {
                                break;
                            }
                        }
                        self.expect_token(Token::RParen)?;
                    }
                    Expr::FunctionCall { name, args }
                } else {
                    Expr::Identifier(name)
                }
            }

            Token::Minus => {
                let (_, r_bp) = prefix_binding_power(UnaryOperator::Minus);
                let rhs = self.parse_expr(r_bp)?;
                Expr::UnaryOp {
                    op: UnaryOperator::Minus,
                    expr: Box::new(rhs),
                }
            }

            Token::Keyword(Keyword::Not) => {
                let (_, r_bp) = prefix_binding_power(UnaryOperator::Not);
                let rhs = self.parse_expr(r_bp)?;
                Expr::UnaryOp {
                    op: UnaryOperator::Not,
                    expr: Box::new(rhs),
                }
            }

            Token::LParen => {
                let expr = self.parse_expr(0)?;
                self.expect_token(Token::RParen)?;
                expr
            }

            other => return Err(format!("Unexpected token in expression: {:?}", other)),
        };

        while let Some((op, l_bp, r_bp)) = infix_binding_power(self.peek()) {
            if l_bp < min_bp {
                break;
            }
            self.advance(); // consume operator token
            let rhs = self.parse_expr(r_bp)?;
            lhs = Expr::BinaryOp {
                left: Box::new(lhs),
                op,
                right: Box::new(rhs),
            };
        }
        let _ = self.peek_nth(0);

        Ok(lhs)
    }
}
fn prefix_binding_power(op: UnaryOperator) -> ((), u8) {
    match op {
        UnaryOperator::Not => ((), 5),
        UnaryOperator::Minus => ((), 15),
    }
}

fn infix_binding_power(tok: &Token) -> Option<(BinaryOperator, u8, u8)> {
    match tok {
        Token::Keyword(Keyword::Or) => Some((BinaryOperator::Or, 1, 2)),
        Token::Keyword(Keyword::And) => Some((BinaryOperator::And, 3, 4)),
        Token::Eq => Some((BinaryOperator::Eq, 7, 8)),
        Token::NotEq => Some((BinaryOperator::NotEq, 7, 8)),
        Token::Lt => Some((BinaryOperator::Lt, 7, 8)),
        Token::Gt => Some((BinaryOperator::Gt, 7, 8)),
        Token::LtEq => Some((BinaryOperator::LtEq, 7, 8)),
        Token::GtEq => Some((BinaryOperator::GtEq, 7, 8)),
        Token::Plus => Some((BinaryOperator::Plus, 9, 10)),
        Token::Minus => Some((BinaryOperator::Minus, 9, 10)),
        Token::Star => Some((BinaryOperator::Star, 11, 12)),
        Token::Slash => Some((BinaryOperator::Slash, 11, 12)),
        _ => None,
    }
}
