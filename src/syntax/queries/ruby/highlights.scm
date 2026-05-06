(comment) @comment
(string) @string
(integer) @number
(float) @number
["def" "end" "class" "module" "if" "elsif" "else" "unless" "while"
 "until" "for" "do" "return" "begin" "rescue" "ensure"
 "yield" "nil" "and" "or" "not" "in"] @keyword
(method name: (identifier) @function)
(call method: (identifier) @function)
(class name: (constant) @type)
(constant) @type
