(comment) @comment
(string) @string
(multiline_string) @string
(character) @string
(integer) @number
(float) @number
["fn" "return" "if" "else" "while" "for" "break" "continue"
 "const" "var" "pub" "struct" "enum" "union" "switch" "unreachable"
 "comptime" "inline" "defer" "errdefer" "try" "catch" "orelse"
 "test" "async" "await" "suspend" "resume" "error"
 "true" "false" "null" "undefined"] @keyword
(function_declaration name: (identifier) @function)
(call_expression function: (identifier) @function)
