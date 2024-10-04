[global.hooks]
run-before = [
  "sh -c 'echo Running global hooks before > tests/generated/${{filename}}.log'",
]
run-after = [
  "sh -c 'echo Running global hooks after >> tests/generated/${{filename}}.log'",
]
run-failed = [
  "sh -c 'echo Running global hooks failed >> tests/generated/${{filename}}.log'",
]
run-finally = [
  "sh -c 'echo Running global hooks finally >> tests/generated/${{filename}}.log'",
]
