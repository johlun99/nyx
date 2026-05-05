(comment) @comment
(string) @string
(template_string) @string
(number) @number
["function" "const" "let" "var" "if" "else" "for" "while" "do" "return"
 "import" "export" "from" "class" "new" "async" "await"
 "try" "catch" "finally" "throw" "typeof" "instanceof"
 "switch" "case" "break" "continue" "default"
 "of" "in" "yield" "delete" "void"] @keyword
(function_declaration name: (identifier) @function)
(call_expression function: (identifier) @function)
(class_declaration name: (identifier) @type)
