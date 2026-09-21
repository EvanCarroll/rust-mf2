| locale | bytes | Catalog::new allocs / reallocs / bytes allocated | simple: allocs over all simple ids | 1-arg pattern (reused String) allocs / op | 1-arg pattern (new String::new()) allocs+reallocs / op | 1-arg pattern (String::with_capacity(128)) allocs / op | select (reused) allocs / op |
|---|---|---|---|---|---|---|---|
| en | 50867 | 0 / 0 / 0 | 0 over 1256 | 0.00 | 3.07 | 1.02 | 0.00 |
| pl | 67302 | 0 / 0 / 0 | 0 over 1256 | 0.00 | 3.09 | 1.03 | 0.00 |
| en-XA | 100528 | 0 / 0 / 0 | 0 over 1256 | 0.00 | 3.18 | 1.15 | 0.00 |
| ar-XB | 60790 | 0 / 0 / 0 | 0 over 1256 | 0.00 | 3.10 | 1.02 | 0.00 |
