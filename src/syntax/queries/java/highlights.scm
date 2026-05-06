(line_comment) @comment
(block_comment) @comment
(string_literal) @string
(character_literal) @string
(decimal_integer_literal) @number
(decimal_floating_point_literal) @number
["if" "else" "switch" "case" "default" "while" "for" "do" "return"
 "break" "continue" "class" "interface" "enum" "extends" "implements"
 "new" "import" "package" "public" "private" "protected" "static"
 "final" "abstract" "try" "catch" "finally" "throw" "throws"
 "synchronized" "volatile" "transient" "instanceof"] @keyword
(method_declaration name: (identifier) @function)
(method_invocation name: (identifier) @function)
(class_declaration name: (identifier) @type)
(interface_declaration name: (identifier) @type)
(type_identifier) @type
