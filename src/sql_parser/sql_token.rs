
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Reserved Words (normalized to uppercase during lexing
    Keyword(Keyword),

    ///  User-Defined Names (table names, column names, aliases)
    Identifier(String),

    /// Literal Values Number
    Number(i64),
    /// Literal Values String
    StringLiteral(String),

    /// Operators =
    Eq,
    /// Operators != or <>
    NotEq,
    /// Operators <
    Lt,
    /// Operators >
    Gt,
    /// Operators <=
    LtEq,
    /// Operators >=
    GtEq,
    /// Operators +
    Plus,
    /// Operators -
    Minus,
    /// Operators * for multiplication or SELECT *
    Star,

    /// Structural punctuation ,
    Comma,
    /// Structural punctuation ;
    Semicolon,
    /// Structural punctuation (
    LParen,
    /// Structural punctuation )
    RParen,
    Slash,

    /// End of file
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Select,
    From,
    Where,
    Limit,
    Insert,
    Into,
    Values,
    Create,
    Table,
    If,
    Exists,
    Primary,
    Key,
    Int,
    Text,
    Bool,
    Update,
    Set,
    Delete,
    As,
    And,
    Or,
    Not,
    True,
    False,
    Null,
}


#[derive(Debug, PartialEq)]
pub enum Statement {
    Select(SelectStmt),
    Insert(InsertStmt),
    CreateTable(CreateTableStmt),
    Update(UpdateStmt),
    Delete(DeleteStmt),
}

#[derive(Debug, PartialEq)]
pub struct SelectStmt {
    pub columns: Vec<SelectItem>,
    pub from: String,
    pub where_clause: Option<Expr>,
    pub limit: Option<usize>,
}

#[derive(Debug, PartialEq)]
pub enum SelectItem {
    Wildcard,
    Expr(Expr),
    AliasedExpr {
        expr: Expr,
        alias: String,
    },
}

#[derive(Debug, PartialEq)]
pub struct InsertStmt {
    pub table: String,
    pub columns: Option<Vec<String>>,
    pub values: Vec<Vec<Expr>>,
}

#[derive(Debug, PartialEq)]
pub struct CreateTableStmt {
    pub table: String,
    pub if_not_exists: bool,
    pub columns: Vec<ColumnDef>,
}

#[derive(Debug, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub is_primary_key: bool,
    pub is_nullable: bool,
}

#[derive(Debug, PartialEq)]
pub enum DataType {
    Int,
    Text,
    Bool,
}

#[derive(Debug, PartialEq)]
pub struct UpdateStmt {
    pub table: String,
    pub assignments: Vec<Assignment>,
    pub where_clause: Option<Expr>,
}

#[derive(Debug, PartialEq)]
pub struct Assignment {
    pub column: String,
    pub value: Expr,
}
#[derive(Debug, PartialEq)]
pub struct DeleteStmt {
    pub table: String,
    pub where_clause: Option<Expr>,
}

#[derive(Debug, PartialEq)]
pub enum Expr {
    Identifier(String),       // e.g., column name `age`
    Literal(LiteralValue),    // e.g., `42`, `'Alice'`, `true`

    // Recursive nodes (must use Box<Expr> for fixed stack size)
    UnaryOp {
        op: UnaryOperator,    // e.g., `NOT active` or `-5`
        expr: Box<Expr>,
    },
    BinaryOp {
        left: Box<Expr>,      // e.g., `age >= 18 AND active = true`
        op: BinaryOperator,
        right: Box<Expr>,
    },
    FunctionCall {
        name: String,         // e.g., `COUNT(*)` or `LOWER(name)`
        args: Vec<Expr>,      // Vec already heap-allocates, so no Box needed!
    },
}

#[derive(Debug, PartialEq)]
pub enum LiteralValue {
    Integer(i64),
    String(String),
    Boolean(bool),
    Null,
}

#[derive(Debug, PartialEq)]
pub enum BinaryOperator {
    Eq,       // =
    NotEq,    // != or <>
    Lt,       // <
    Gt,       // >
    LtEq,     // <=
    GtEq,     // >=
    And,      // AND
    Or,       // OR
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
}

#[derive(Debug, PartialEq)]
pub enum UnaryOperator {
    Not,      // NOT
    Minus,    // -
}
