(comment) @comment
(string) @string
(interpolated_string) @string
(integer_literal) @number
(floating_point_literal) @number
["def" "val" "var" "if" "else" "match" "case" "for" "while" "do"
 "return" "class" "object" "trait" "extends" "with" "import"
 "package" "new" "type" "sealed" "abstract" "final" "override"
 "private" "protected" "implicit" "lazy" "yield" "try" "catch"
 "finally" "throw" "true" "false"] @keyword
(function_definition name: (identifier) @function)
(call_expression function: (identifier) @function)
(class_definition name: (identifier) @type)
(type_identifier) @type
