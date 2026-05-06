(comment) @comment
(string) @string
(char) @string
(integer) @number
(float) @number
["module" "where" "import" "qualified" "as" "hiding" "data" "newtype"
 "type" "class" "instance" "deriving" "if" "then" "else" "case" "of"
 "let" "in" "do" "where" "infixl" "infixr" "infix"
 "forall" "foreign"] @keyword
(function name: (variable) @function)
(constructor) @type
