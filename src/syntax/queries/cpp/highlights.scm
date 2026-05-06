(comment) @comment
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string
(number_literal) @number
["if" "else" "switch" "case" "default" "while" "for" "do" "return"
 "break" "continue" "class" "struct" "enum" "namespace" "template"
 "typename" "typedef" "const" "static" "virtual" "override"
 "public" "private" "protected" "new" "delete" "throw" "try" "catch"
 "using" "inline" "extern" "volatile" "constexpr"] @keyword
(function_declarator declarator: (identifier) @function)
(call_expression function: (identifier) @function)
(type_identifier) @type
(primitive_type) @type
(sized_type_specifier) @type
