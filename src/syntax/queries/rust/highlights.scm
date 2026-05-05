(line_comment) @comment
(block_comment) @comment
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string
(integer_literal) @number
(float_literal) @number
["fn" "let" "if" "else" "match" "for" "while" "loop" "return"
 "use" "mod" "pub" "struct" "enum" "impl" "trait" "type" "const"
 "static" "async" "await" "where" "true" "false"
 "as" "in" "ref" "move" "break" "continue" "unsafe" "extern" "dyn"] @keyword
(function_item name: (identifier) @function)
(call_expression function: (identifier) @function)
(type_identifier) @type
(primitive_type) @type
