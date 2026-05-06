(comment) @comment
(string) @string
(encapsed_string) @string
(heredoc) @string
(integer) @number
(float) @number
["if" "else" "elseif" "while" "for" "foreach" "do" "switch" "case"
 "default" "return" "break" "continue" "function" "class" "interface"
 "extends" "implements" "new" "echo" "print" "public" "private"
 "protected" "static" "abstract" "final" "const" "use"
 "namespace" "try" "catch" "finally" "throw" "yield"] @keyword
(function_definition name: (name) @function)
(function_call_expression function: (name) @function)
(class_declaration name: (name) @type)
(named_type (name) @type)
