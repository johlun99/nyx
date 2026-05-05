(comment) @comment
(string) @string
(concatenated_string) @string
(integer) @number
(float) @number
["def" "class" "if" "elif" "else" "for" "while" "return" "import" "from"
 "as" "with" "try" "except" "finally" "raise" "pass" "break" "continue"
 "and" "or" "not" "in" "is" "lambda" "yield"
 "global" "nonlocal" "del" "assert" "async" "await"] @keyword
(function_definition name: (identifier) @function)
(call function: (identifier) @function)
(class_definition name: (identifier) @type)
