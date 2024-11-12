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

[repository.hooks]
run-before = [
  "sh -c 'echo Running repository hooks before >> tests/generated/${{filename}}.log'",
]
run-after = [
  "sh -c 'echo Running repository hooks after >> tests/generated/${{filename}}.log'",
]
run-failed = [
  "sh -c 'echo Running repository hooks failed >> tests/generated/${{filename}}.log'",
]
run-finally = [
  "sh -c 'echo Running repository hooks finally >> tests/generated/${{filename}}.log'",
]

[backup.hooks]
run-before = [
  "sh -c 'echo Running backup hooks before >> tests/generated/${{filename}}.log'",
]
run-after = [
  "sh -c 'echo Running backup hooks after >> tests/generated/${{filename}}.log'",
]
run-failed = [
  "sh -c 'echo Running backup hooks failed >> tests/generated/${{filename}}.log'",
]
run-finally = [
  "sh -c 'echo Running backup hooks finally >> tests/generated/${{filename}}.log'",
]
