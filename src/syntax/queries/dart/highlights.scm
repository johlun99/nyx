(comment) @comment
(string_literal) @string
(decimal_integer_literal) @number
(decimal_floating_point_literal) @number
(hex_integer_literal) @number
["if" "else" "switch" "case" "default" "while" "for" "do" "return"
 "break" "continue" "class" "extends" "implements" "with" "new"
 "import" "export" "library" "part" "typedef" "enum" "abstract"
 "final" "const" "static" "var" "async" "await" "yield"
 "try" "catch" "finally" "throw" "rethrow" "is" "as" "in"
 "this" "super" "late" "required"] @keyword
(function_signature name: (identifier) @function)
(type_identifier) @type
