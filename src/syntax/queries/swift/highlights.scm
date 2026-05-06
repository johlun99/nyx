(comment) @comment
(line_string_literal) @string
(multi_line_string_literal) @string
(integer_literal) @number
(real_literal) @number
["func" "let" "var" "if" "guard" "switch" "case"
 "for" "while" "repeat" "return" "break" "continue" "class" "struct"
 "enum" "protocol" "extension" "import" "typealias" "self"
 "true" "false" "nil" "try" "as" "is"
 "in" "async" "await" "public" "private" "internal"
 "static" "override" "init" "deinit"] @keyword
(function_declaration (simple_identifier) @function)
(call_expression (simple_identifier) @function)
(class_declaration name: (type_identifier) @type)
(type_identifier) @type
