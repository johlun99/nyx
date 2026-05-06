(comment) @comment
(string) @string
(raw_string) @string
(heredoc_body) @string
["if" "then" "else" "elif" "fi" "case" "esac" "for" "while" "do"
 "done" "in" "function" "local" "export" "readonly"
 "unset" "declare" "typeset" "select" "until"] @keyword
(function_definition name: (word) @function)
(command name: (command_name (word) @function))
