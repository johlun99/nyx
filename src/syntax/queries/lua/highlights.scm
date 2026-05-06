(comment) @comment
(string) @string
(number) @number
["if" "then" "else" "elseif" "end" "do" "while" "repeat" "until"
 "for" "in" "return" "local" "function" "and" "or" "not"
 "goto"] @keyword
(function_declaration name: (identifier) @function)
(function_call name: (identifier) @function)
