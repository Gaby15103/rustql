use rustql::sql_parser::*;

pub fn parse_script(sql: &str) -> Result<Vec<Statement>, String> {
    let mut lexer = Lexer::new(sql);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse_statements()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_table_with_constraints() {
        let sql = "CREATE TABLE IF NOT EXISTS users (
            id INT PRIMARY KEY,
            name TEXT NOT NULL,
            active BOOL
        );";
        let stmts = parse_script(sql).unwrap();
        assert_eq!(
            stmts,
            vec![Statement::CreateTable(CreateTableStmt {
                table: "users".into(),
                if_not_exists: true,
                columns: vec![
                    ColumnDef {
                        name: "id".into(),
                        data_type: DataType::Int,
                        is_primary_key: true,
                        is_nullable: false,
                    },
                    ColumnDef {
                        name: "name".into(),
                        data_type: DataType::Text,
                        is_primary_key: false,
                        is_nullable: false,
                    },
                    ColumnDef {
                        name: "active".into(),
                        data_type: DataType::Bool,
                        is_primary_key: false,
                        is_nullable: true,
                    },
                ],
            })]
        );
    }

    #[test]
    fn test_operator_precedence() {
        // `1 + 2 * 3` must parse as `1 + (2 * 3)`
        let sql = "SELECT 1 + 2 * 3 FROM dual;";
        let stmts = parse_script(sql).unwrap();
        let expected_expr = Expr::BinaryOp {
            left: Box::new(Expr::Literal(LiteralValue::Integer(1))),
            op: BinaryOperator::Plus,
            right: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Literal(LiteralValue::Integer(2))),
                op: BinaryOperator::Star,
                right: Box::new(Expr::Literal(LiteralValue::Integer(3))),
            }),
        };

        assert_eq!(
            stmts,
            vec![Statement::Select(SelectStmt {
                columns: vec![SelectItem::Expr(expected_expr)],
                from: "dual".into(),
                where_clause: None,
                limit: None,
                order_by: None,
            })]
        );
    }

    #[test]
    fn test_update_and_delete() {
        let sql = "
            -- Update user age
            UPDATE users SET age = age + 1, name = 'O''Connor' WHERE id = 42;
            DELETE FROM users WHERE active = false;
        ";
        let stmts = parse_script(sql).unwrap();
        assert_eq!(stmts.len(), 2);
        assert!(matches!(stmts[0], Statement::Update(_)));
        assert!(matches!(stmts[1], Statement::Delete(_)));
    }

    #[test]
    fn test_syntax_errors() {
        assert!(parse_script("SELECT FROM users").is_err());
        assert!(parse_script("SELECT 'unterminated FROM users").is_err());
        assert!(parse_script("UPDATE users SET a = 1 WHERE (x = 1").is_err());
    }
}