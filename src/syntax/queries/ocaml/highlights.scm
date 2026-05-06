(comment) @comment
(string) @string
(character) @string
(number) @number
["let" "in" "if" "then" "else" "match" "with" "fun" "function"
 "type" "module" "struct" "sig" "end" "open" "include" "val"
 "and" "rec" "mutable" "begin" "for" "do" "done" "while" "to"
 "downto" "try" "exception" "of" "true" "false"
 "when" "as" "external" "assert" "lazy"] @keyword
(application_expression function: (value_path (value_name) @function))
(type_constructor_path (type_constructor) @type)
(constructor_name) @type
