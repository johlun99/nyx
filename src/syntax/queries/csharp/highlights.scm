(comment) @comment
(string_literal) @string
(verbatim_string_literal) @string
(interpolated_string_expression) @string
(character_literal) @string
(integer_literal) @number
(real_literal) @number
["if" "else" "switch" "case" "default" "while" "for" "foreach" "do"
 "return" "break" "continue" "class" "struct" "interface" "enum"
 "namespace" "using" "public" "private" "protected" "internal"
 "static" "virtual" "override" "abstract" "sealed" "new" "try"
 "catch" "finally" "throw" "async" "await" "var" "const" "readonly"] @keyword
(method_declaration name: (identifier) @function)
(invocation_expression function: (identifier) @function)
(class_declaration name: (identifier) @type)
(interface_declaration name: (identifier) @type)
(predefined_type) @type
