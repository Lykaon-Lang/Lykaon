# M5e â€” five real Lykaon programs

Each program is written the way a normal user would write it. They exercise
tables, databases, servers, tasks/channels, tools and text patterns.

| App | File | What it shows |
|---|---|---|
| Web server + SQLite | `web-server.lykaon` | `Open database`, `Serve`, `When someone gets`, `Respond` |
| Data report | `data-report.lykaon` | `Load`, statistics, grouping, `Plot` |
| CLI tool | `cli-tool.lykaon` | `Takes`, automatic help |
| Background worker | `worker.lykaon` | tasks and channels |
| Text tool | `text-tool.lykaon` | text patterns and counts |

## Run

```
lykaon-cli run data-report.lykaon
lykaon-cli run worker.lykaon
lykaon-cli run text-tool.lykaon
lykaon-cli run cli-tool.lykaon -- Lykaon
lykaon-cli run web-server.lykaon        # then: curl http://127.0.0.1:8080/hello
```

`data-report.lykaon` expects a `data/sales.csv` (the Chapter 54 table);
`web-server.lykaon` creates `shopdata` on first run.

## Expected

- data report: `rows 8`, `average price 111.88`, the region totals, and
  `output/plot-1.svg`.
- worker: `1`, `2`, `3` (tasks may interleave; the channel keeps order).
- text tool: `3` and `cat,sat,down`.
- CLI tool: `hello Lykaon`.
- web server: `hello` at `/hello`.

See `APPS_FINDINGS.md` for compiler issues found while writing these.