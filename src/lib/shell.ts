/** Joins argv into a copy-pasteable POSIX shell command line, quoting only where needed. */
export function shellJoin(argv: string[]): string {
  return argv
    .map((arg) => {
      if (arg === "") return "''";
      if (/^[\w@%+=:,./-]+$/.test(arg)) return arg;
      return `'${arg.replace(/'/g, `'\\''`)}'`;
    })
    .join(" ");
}
