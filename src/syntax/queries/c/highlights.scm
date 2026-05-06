(comment) @comment
(string_literal) @string
(system_lib_string) @string
(char_literal) @string
(number_literal) @number
["if" "else" "switch" "case" "default" "while" "for" "do" "return"
 "break" "continue" "goto" "typedef" "struct" "union" "enum"
 "sizeof" "static" "extern" "const" "volatile" "inline"] @keyword
(function_declarator declarator: (identifier) @function)
(call_expression function: (identifier) @function)
(type_identifier) @type
(primitive_type) @type
(sized_type_specifier) @type
