(comment) @comment
(interpreted_string_literal) @string
(raw_string_literal) @string
(rune_literal) @string
(int_literal) @number
(float_literal) @number
["func" "return" "if" "else" "for" "range" "switch" "case" "default"
 "break" "continue" "go" "defer" "select" "chan" "map" "struct"
 "interface" "package" "import" "type" "var" "const" "fallthrough"
 "goto"] @keyword
(function_declaration name: (identifier) @function)
(call_expression function: (identifier) @function)
(type_identifier) @type
