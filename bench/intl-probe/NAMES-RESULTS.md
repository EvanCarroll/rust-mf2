# The number split against the Rust path — results

Command: `bench/intl-probe/scripts/7-names.sh`, started 2026-10-05T01:47:28Z. Built from crates/ of the working tree at c59364a (0 changed paths under crates/). Machine: 8 CPUs, 2117.501 MHz at the end, load 1.72 1.39 1.53.
Written by the command; not edited by hand. `rt-names-cu`: `:currency` and `:unit` take their names from
`Intl.NumberFormat`, the digits, rounding and plural selection stay in Rust (`mf2-fn-number/intl-names`,
`mf2-host-web/intl-names`). `rust-cu`: the Rust path. `rt-intl-cu`: the whole `intl` option, for reference.

## 1. The client: wasm + JS glue, gzip -9, over `base`

| Variant | wasm gz | JS gz | over `base` | against `rust-cu` |
|---|---:|---:|---:|---:|
| `rust-cu` | 48,215 | 3,710 | +12,756 |  |
| `rt-names-cu` | 47,345 | 4,698 | +12,874 | +118 |
| `rt-intl-cu` | 43,519 | 4,698 | +9,048 | -3,708 |

## 2. A catalog per language: the panel's `:currency` and `:unit` messages, brotli (q 11, window 22)

| Locale | `currency.data` raw B | `unit.data` raw B | catalog br, Rust path | the split | saved |
|---|---:|---:|---:|---:|---:|
| en | 127 | 396 | 1,251 | 999 | -252 |
| es | 186 | 410 | 1,279 | 963 | -316 |
| de | 113 | 281 | 1,179 | 952 | -227 |
| fr | 185 | 437 | 1,289 | 999 | -290 |
| ar | 104 | 433 | 1,300 | 1,016 | -284 |
| he | 96 | 396 | 1,250 | 1,000 | -250 |
| ja | 84 | 352 | 1,247 | 977 | -270 |
| hi | 122 | 684 | 1,276 | 979 | -297 |
| ru | 279 | 608 | 1,373 | 986 | -387 |
| pl | 249 | 572 | 1,374 | 1,004 | -370 |
| cy | 123 | 483 | 1,292 | 992 | -300 |

## 3. Time per placeholder (ns, medians; the two variants alternated in one page)

| Engine | family | `rust-cu` ns, median of rows | `rt-names-cu` ns | ratio, range over rows |
|---|---|---:|---:|---|
| chromium 143.0.7499.4 | `:currency` | 1,988 | 12,506 | 3.98–7.39× |
| chromium 143.0.7499.4 | `:unit` | 2,926 | 9,574 | 2.51–4.34× |
| firefox 155.0 | `:currency` | 2,551 | 13,302 | 3.42–6.40× |
| firefox 155.0 | `:unit` | 3,547 | 11,792 | 2.65–4.52× |

Per locale and annotation (median ns, `rust-cu` → `rt-names-cu`):

<details><summary>chromium 143.0.7499.4</summary>

| Locale | annotation | `rust-cu` | `rt-names-cu` | ratio |
|---|---|---:|---:|---:|
| en | `:currency currency=EUR` | 1,925 | 12,748 | 6.62× |
| en | `:currency currency=USD` | 1,694 | 11,408 | 6.74× |
| en | `:currency currency=JPY` | 1,286 | 9,135 | 7.10× |
| en | `:currency currency=EUR currencyDisplay=code` | 1,985 | 12,506 | 6.30× |
| en | `:currency currency=EUR currencyDisplay=name` | 2,232 | 13,633 | 6.11× |
| en | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,025 | 12,742 | 6.29× |
| en | `:currency currency=EUR currencySign=accounting` | 2,049 | 13,648 | 6.66× |
| en | `:currency currency=EUR fractionDigits=0` | 1,553 | 6,905 | 4.45× |
| en | `:unit unit=kilometer` | 2,565 | 7,550 | 2.94× |
| en | `:unit unit=kilometer unitDisplay=long` | 2,796 | 8,123 | 2.90× |
| en | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 2,767 | 9,847 | 3.56× |
| en | `:unit unit=celsius` | 2,389 | 8,323 | 3.48× |
| en | `:unit unit=kilogram unitDisplay=long` | 2,831 | 8,281 | 2.93× |
| en | `:unit unit=liter unitDisplay=long` | 2,740 | 8,062 | 2.94× |
| es | `:currency currency=EUR` | 1,490 | 9,895 | 6.64× |
| es | `:currency currency=USD` | 1,595 | 10,493 | 6.58× |
| es | `:currency currency=JPY` | 1,499 | 8,774 | 5.85× |
| es | `:currency currency=EUR currencyDisplay=code` | 1,720 | 10,556 | 6.14× |
| es | `:currency currency=EUR currencyDisplay=name` | 1,948 | 10,275 | 5.27× |
| es | `:currency currency=USD currencyDisplay=narrowSymbol` | 1,828 | 10,653 | 5.83× |
| es | `:currency currency=EUR currencySign=accounting` | 1,702 | 10,712 | 6.29× |
| es | `:currency currency=EUR fractionDigits=0` | 1,429 | 5,727 | 4.01× |
| es | `:unit unit=kilometer` | 2,267 | 6,822 | 3.01× |
| es | `:unit unit=kilometer unitDisplay=long` | 2,516 | 8,388 | 3.33× |
| es | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 2,467 | 8,772 | 3.56× |
| es | `:unit unit=celsius` | 2,214 | 7,712 | 3.48× |
| es | `:unit unit=kilogram unitDisplay=long` | 2,526 | 7,323 | 2.90× |
| es | `:unit unit=liter unitDisplay=long` | 2,461 | 7,233 | 2.94× |
| de | `:currency currency=EUR` | 1,426 | 10,532 | 7.39× |
| de | `:currency currency=USD` | 1,455 | 10,386 | 7.14× |
| de | `:currency currency=JPY` | 1,473 | 9,057 | 6.15× |
| de | `:currency currency=EUR currencyDisplay=code` | 1,646 | 10,808 | 6.57× |
| de | `:currency currency=EUR currencyDisplay=name` | 1,729 | 10,162 | 5.88× |
| de | `:currency currency=USD currencyDisplay=narrowSymbol` | 1,660 | 10,811 | 6.51× |
| de | `:currency currency=EUR currencySign=accounting` | 1,635 | 11,272 | 6.89× |
| de | `:currency currency=EUR fractionDigits=0` | 1,414 | 5,866 | 4.15× |
| de | `:unit unit=kilometer` | 2,049 | 6,620 | 3.23× |
| de | `:unit unit=kilometer unitDisplay=long` | 2,264 | 7,201 | 3.18× |
| de | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 2,328 | 8,845 | 3.80× |
| de | `:unit unit=celsius` | 2,010 | 7,480 | 3.72× |
| de | `:unit unit=kilogram unitDisplay=long` | 2,299 | 7,103 | 3.09× |
| de | `:unit unit=liter unitDisplay=long` | 2,296 | 7,090 | 3.09× |
| fr | `:currency currency=EUR` | 1,794 | 12,393 | 6.91× |
| fr | `:currency currency=USD` | 1,589 | 10,283 | 6.47× |
| fr | `:currency currency=JPY` | 1,520 | 8,993 | 5.92× |
| fr | `:currency currency=EUR currencyDisplay=code` | 1,708 | 10,818 | 6.33× |
| fr | `:currency currency=EUR currencyDisplay=name` | 1,908 | 10,534 | 5.52× |
| fr | `:currency currency=USD currencyDisplay=narrowSymbol` | 1,839 | 10,920 | 5.94× |
| fr | `:currency currency=EUR currencySign=accounting` | 1,783 | 10,986 | 6.16× |
| fr | `:currency currency=EUR fractionDigits=0` | 1,460 | 5,882 | 4.03× |
| fr | `:unit unit=kilometer` | 2,365 | 7,824 | 3.31× |
| fr | `:unit unit=kilometer unitDisplay=long` | 2,551 | 8,298 | 3.25× |
| fr | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 2,523 | 9,155 | 3.63× |
| fr | `:unit unit=celsius` | 2,259 | 7,859 | 3.48× |
| fr | `:unit unit=kilogram unitDisplay=long` | 2,537 | 8,233 | 3.25× |
| fr | `:unit unit=liter unitDisplay=long` | 2,538 | 8,454 | 3.33× |
| ar | `:currency currency=EUR` | 1,574 | 11,006 | 6.99× |
| ar | `:currency currency=USD` | 1,585 | 10,942 | 6.90× |
| ar | `:currency currency=JPY` | 1,537 | 9,518 | 6.19× |
| ar | `:currency currency=EUR currencyDisplay=code` | 1,776 | 11,186 | 6.30× |
| ar | `:currency currency=EUR currencyDisplay=name` | 1,797 | 11,265 | 6.27× |
| ar | `:currency currency=USD currencyDisplay=narrowSymbol` | 1,791 | 11,336 | 6.33× |
| ar | `:currency currency=EUR currencySign=accounting` | 1,700 | 11,083 | 6.52× |
| ar | `:currency currency=EUR fractionDigits=0` | 1,526 | 6,296 | 4.13× |
| ar | `:unit unit=kilometer` | 2,448 | 8,842 | 3.61× |
| ar | `:unit unit=kilometer unitDisplay=long` | 2,685 | 9,299 | 3.46× |
| ar | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 2,551 | 11,074 | 4.34× |
| ar | `:unit unit=celsius` | 2,172 | 8,482 | 3.90× |
| ar | `:unit unit=kilogram unitDisplay=long` | 2,685 | 9,405 | 3.50× |
| ar | `:unit unit=liter unitDisplay=long` | 2,696 | 9,009 | 3.34× |
| he | `:currency currency=EUR` | 1,925 | 13,065 | 6.79× |
| he | `:currency currency=USD` | 1,826 | 12,361 | 6.77× |
| he | `:currency currency=JPY` | 1,724 | 10,889 | 6.32× |
| he | `:currency currency=EUR currencyDisplay=code` | 2,057 | 13,004 | 6.32× |
| he | `:currency currency=EUR currencyDisplay=name` | 2,059 | 12,577 | 6.11× |
| he | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,055 | 13,084 | 6.37× |
| he | `:currency currency=EUR currencySign=accounting` | 2,025 | 13,109 | 6.47× |
| he | `:currency currency=EUR fractionDigits=0` | 1,761 | 7,148 | 4.06× |
| he | `:unit unit=kilometer` | 2,661 | 8,964 | 3.37× |
| he | `:unit unit=kilometer unitDisplay=long` | 2,948 | 9,601 | 3.26× |
| he | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 2,814 | 11,559 | 4.11× |
| he | `:unit unit=celsius` | 2,731 | 9,387 | 3.44× |
| he | `:unit unit=kilogram unitDisplay=long` | 3,433 | 11,101 | 3.23× |
| he | `:unit unit=liter unitDisplay=long` | 3,285 | 11,352 | 3.46× |
| ja | `:currency currency=EUR` | 2,040 | 14,705 | 7.21× |
| ja | `:currency currency=USD` | 2,026 | 13,828 | 6.82× |
| ja | `:currency currency=JPY` | 1,999 | 12,534 | 6.27× |
| ja | `:currency currency=EUR currencyDisplay=code` | 2,447 | 15,380 | 6.29× |
| ja | `:currency currency=EUR currencyDisplay=name` | 2,485 | 15,665 | 6.30× |
| ja | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,416 | 15,122 | 6.26× |
| ja | `:currency currency=EUR currencySign=accounting` | 2,504 | 16,437 | 6.56× |
| ja | `:currency currency=EUR fractionDigits=0` | 2,046 | 8,300 | 4.06× |
| ja | `:unit unit=kilometer` | 3,197 | 9,775 | 3.06× |
| ja | `:unit unit=kilometer unitDisplay=long` | 3,541 | 11,978 | 3.38× |
| ja | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,358 | 12,677 | 3.78× |
| ja | `:unit unit=celsius` | 2,926 | 10,696 | 3.65× |
| ja | `:unit unit=kilogram unitDisplay=long` | 3,575 | 11,949 | 3.34× |
| ja | `:unit unit=liter unitDisplay=long` | 3,351 | 11,557 | 3.45× |
| hi | `:currency currency=EUR` | 2,047 | 14,893 | 7.28× |
| hi | `:currency currency=USD` | 2,046 | 13,982 | 6.83× |
| hi | `:currency currency=JPY` | 1,988 | 12,582 | 6.33× |
| hi | `:currency currency=EUR currencyDisplay=code` | 2,431 | 15,307 | 6.30× |
| hi | `:currency currency=EUR currencyDisplay=name` | 2,414 | 15,053 | 6.24× |
| hi | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,345 | 14,545 | 6.20× |
| hi | `:currency currency=EUR currencySign=accounting` | 2,216 | 14,860 | 6.70× |
| hi | `:currency currency=EUR fractionDigits=0` | 1,912 | 7,851 | 4.11× |
| hi | `:unit unit=kilometer` | 3,598 | 11,580 | 3.22× |
| hi | `:unit unit=kilometer unitDisplay=long` | 3,869 | 12,242 | 3.16× |
| hi | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,746 | 14,304 | 3.82× |
| hi | `:unit unit=celsius` | 3,060 | 10,619 | 3.47× |
| hi | `:unit unit=kilogram unitDisplay=long` | 4,043 | 11,838 | 2.93× |
| hi | `:unit unit=liter unitDisplay=long` | 3,479 | 11,658 | 3.35× |
| ru | `:currency currency=EUR` | 2,084 | 14,665 | 7.04× |
| ru | `:currency currency=USD` | 2,370 | 14,354 | 6.06× |
| ru | `:currency currency=JPY` | 2,415 | 12,551 | 5.20× |
| ru | `:currency currency=EUR currencyDisplay=code` | 2,334 | 14,821 | 6.35× |
| ru | `:currency currency=EUR currencyDisplay=name` | 2,635 | 14,872 | 5.64× |
| ru | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,666 | 15,048 | 5.65× |
| ru | `:currency currency=EUR currencySign=accounting` | 2,308 | 15,054 | 6.52× |
| ru | `:currency currency=EUR fractionDigits=0` | 2,018 | 8,379 | 4.15× |
| ru | `:unit unit=kilometer` | 3,468 | 11,239 | 3.24× |
| ru | `:unit unit=kilometer unitDisplay=long` | 3,783 | 11,982 | 3.17× |
| ru | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,819 | 14,453 | 3.78× |
| ru | `:unit unit=celsius` | 3,390 | 10,699 | 3.16× |
| ru | `:unit unit=kilogram unitDisplay=long` | 3,774 | 11,296 | 2.99× |
| ru | `:unit unit=liter unitDisplay=long` | 3,529 | 11,296 | 3.20× |
| pl | `:currency currency=EUR` | 2,118 | 14,772 | 6.97× |
| pl | `:currency currency=USD` | 2,528 | 14,639 | 5.79× |
| pl | `:currency currency=JPY` | 2,405 | 12,734 | 5.29× |
| pl | `:currency currency=EUR currencyDisplay=code` | 2,295 | 14,966 | 6.52× |
| pl | `:currency currency=EUR currencyDisplay=name` | 2,594 | 13,845 | 5.34× |
| pl | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,704 | 14,745 | 5.45× |
| pl | `:currency currency=EUR currencySign=accounting` | 2,376 | 15,124 | 6.37× |
| pl | `:currency currency=EUR fractionDigits=0` | 2,041 | 8,171 | 4.00× |
| pl | `:unit unit=kilometer` | 3,323 | 9,574 | 2.88× |
| pl | `:unit unit=kilometer unitDisplay=long` | 3,656 | 10,593 | 2.90× |
| pl | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 4,053 | 12,513 | 3.09× |
| pl | `:unit unit=celsius` | 3,365 | 9,309 | 2.77× |
| pl | `:unit unit=kilogram unitDisplay=long` | 3,606 | 10,205 | 2.83× |
| pl | `:unit unit=liter unitDisplay=long` | 3,500 | 10,191 | 2.91× |
| cy | `:currency currency=EUR` | 2,005 | 14,176 | 7.07× |
| cy | `:currency currency=USD` | 2,081 | 12,880 | 6.19× |
| cy | `:currency currency=JPY` | 1,909 | 11,634 | 6.09× |
| cy | `:currency currency=EUR currencyDisplay=code` | 2,364 | 14,214 | 6.01× |
| cy | `:currency currency=EUR currencyDisplay=name` | 2,493 | 13,539 | 5.43× |
| cy | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,366 | 13,925 | 5.88× |
| cy | `:currency currency=EUR currencySign=accounting` | 2,378 | 14,816 | 6.23× |
| cy | `:currency currency=EUR fractionDigits=0` | 1,873 | 7,452 | 3.98× |
| cy | `:unit unit=kilometer` | 3,048 | 8,875 | 2.91× |
| cy | `:unit unit=kilometer unitDisplay=long` | 3,523 | 9,747 | 2.77× |
| cy | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,420 | 11,569 | 3.38× |
| cy | `:unit unit=celsius` | 3,149 | 9,887 | 3.14× |
| cy | `:unit unit=kilogram unitDisplay=long` | 3,739 | 9,398 | 2.51× |
| cy | `:unit unit=liter unitDisplay=long` | 3,287 | 9,757 | 2.97× |

</details>

<details><summary>firefox 155.0</summary>

| Locale | annotation | `rust-cu` | `rt-names-cu` | ratio |
|---|---|---:|---:|---:|
| en | `:currency currency=EUR` | 2,721 | 16,259 | 5.97× |
| en | `:currency currency=USD` | 2,566 | 14,271 | 5.56× |
| en | `:currency currency=JPY` | 2,149 | 12,447 | 5.79× |
| en | `:currency currency=EUR currencyDisplay=code` | 2,668 | 14,271 | 5.35× |
| en | `:currency currency=EUR currencyDisplay=name` | 2,777 | 13,138 | 4.73× |
| en | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,513 | 13,029 | 5.18× |
| en | `:currency currency=EUR currencySign=accounting` | 2,367 | 13,605 | 5.75× |
| en | `:currency currency=EUR fractionDigits=0` | 1,970 | 6,737 | 3.42× |
| en | `:unit unit=kilometer` | 2,804 | 8,758 | 3.12× |
| en | `:unit unit=kilometer unitDisplay=long` | 3,045 | 9,314 | 3.06× |
| en | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,109 | 9,225 | 2.97× |
| en | `:unit unit=celsius` | 2,698 | 9,931 | 3.68× |
| en | `:unit unit=kilogram unitDisplay=long` | 3,132 | 9,366 | 2.99× |
| en | `:unit unit=liter unitDisplay=long` | 2,946 | 8,946 | 3.04× |
| es | `:currency currency=EUR` | 2,022 | 12,094 | 5.98× |
| es | `:currency currency=USD` | 2,143 | 11,964 | 5.58× |
| es | `:currency currency=JPY` | 1,958 | 10,582 | 5.41× |
| es | `:currency currency=EUR currencyDisplay=code` | 2,179 | 11,937 | 5.48× |
| es | `:currency currency=EUR currencyDisplay=name` | 4,440 | 19,934 | 4.49× |
| es | `:currency currency=USD currencyDisplay=narrowSymbol` | 3,443 | 17,875 | 5.19× |
| es | `:currency currency=EUR currencySign=accounting` | 3,175 | 17,549 | 5.53× |
| es | `:currency currency=EUR fractionDigits=0` | 2,725 | 9,536 | 3.50× |
| es | `:unit unit=kilometer` | 4,286 | 13,728 | 3.20× |
| es | `:unit unit=kilometer unitDisplay=long` | 4,119 | 14,392 | 3.49× |
| es | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,605 | 10,703 | 2.97× |
| es | `:unit unit=celsius` | 2,938 | 11,233 | 3.82× |
| es | `:unit unit=kilogram unitDisplay=long` | 3,240 | 9,865 | 3.05× |
| es | `:unit unit=liter unitDisplay=long` | 3,135 | 9,812 | 3.13× |
| de | `:currency currency=EUR` | 1,897 | 12,069 | 6.36× |
| de | `:currency currency=USD` | 1,881 | 11,988 | 6.37× |
| de | `:currency currency=JPY` | 1,923 | 10,644 | 5.54× |
| de | `:currency currency=EUR currencyDisplay=code` | 2,112 | 12,335 | 5.84× |
| de | `:currency currency=EUR currencyDisplay=name` | 2,468 | 12,680 | 5.14× |
| de | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,214 | 12,909 | 5.83× |
| de | `:currency currency=EUR currencySign=accounting` | 2,188 | 12,909 | 5.90× |
| de | `:currency currency=EUR fractionDigits=0` | 1,882 | 6,978 | 3.71× |
| de | `:unit unit=kilometer` | 2,647 | 8,764 | 3.31× |
| de | `:unit unit=kilometer unitDisplay=long` | 2,904 | 9,489 | 3.27× |
| de | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,124 | 9,771 | 3.13× |
| de | `:unit unit=celsius` | 3,206 | 12,061 | 3.76× |
| de | `:unit unit=kilogram unitDisplay=long` | 3,483 | 10,921 | 3.14× |
| de | `:unit unit=liter unitDisplay=long` | 3,222 | 10,417 | 3.23× |
| fr | `:currency currency=EUR` | 3,396 | 18,841 | 5.55× |
| fr | `:currency currency=USD` | 4,174 | 24,040 | 5.76× |
| fr | `:currency currency=JPY` | 2,979 | 17,091 | 5.74× |
| fr | `:currency currency=EUR currencyDisplay=code` | 4,152 | 22,165 | 5.34× |
| fr | `:currency currency=EUR currencyDisplay=name` | 4,555 | 22,671 | 4.98× |
| fr | `:currency currency=USD currencyDisplay=narrowSymbol` | 4,962 | 25,414 | 5.12× |
| fr | `:currency currency=EUR currencySign=accounting` | 3,571 | 19,654 | 5.50× |
| fr | `:currency currency=EUR fractionDigits=0` | 2,571 | 9,324 | 3.63× |
| fr | `:unit unit=kilometer` | 3,519 | 12,299 | 3.49× |
| fr | `:unit unit=kilometer unitDisplay=long` | 3,679 | 12,617 | 3.43× |
| fr | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,708 | 11,036 | 2.98× |
| fr | `:unit unit=celsius` | 3,127 | 11,339 | 3.63× |
| fr | `:unit unit=kilogram unitDisplay=long` | 3,624 | 12,765 | 3.52× |
| fr | `:unit unit=liter unitDisplay=long` | 3,387 | 11,946 | 3.53× |
| ar | `:currency currency=EUR` | 2,314 | 13,729 | 5.93× |
| ar | `:currency currency=USD` | 2,371 | 13,900 | 5.86× |
| ar | `:currency currency=JPY` | 2,238 | 12,440 | 5.56× |
| ar | `:currency currency=EUR currencyDisplay=code` | 2,596 | 14,332 | 5.52× |
| ar | `:currency currency=EUR currencyDisplay=name` | 2,708 | 16,830 | 6.21× |
| ar | `:currency currency=USD currencyDisplay=narrowSymbol` | 3,971 | 24,891 | 6.27× |
| ar | `:currency currency=EUR currencySign=accounting` | 3,710 | 21,244 | 5.73× |
| ar | `:currency currency=EUR fractionDigits=0` | 2,847 | 10,021 | 3.52× |
| ar | `:unit unit=kilometer` | 4,124 | 15,346 | 3.72× |
| ar | `:unit unit=kilometer unitDisplay=long` | 5,330 | 17,473 | 3.28× |
| ar | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 5,068 | 22,925 | 4.52× |
| ar | `:unit unit=celsius` | 4,444 | 19,947 | 4.49× |
| ar | `:unit unit=kilogram unitDisplay=long` | 5,945 | 22,327 | 3.76× |
| ar | `:unit unit=liter unitDisplay=long` | 5,996 | 24,041 | 4.01× |
| he | `:currency currency=EUR` | 3,396 | 20,422 | 6.01× |
| he | `:currency currency=USD` | 3,038 | 17,794 | 5.86× |
| he | `:currency currency=JPY` | 2,575 | 14,631 | 5.68× |
| he | `:currency currency=EUR currencyDisplay=code` | 2,671 | 14,984 | 5.61× |
| he | `:currency currency=EUR currencyDisplay=name` | 2,725 | 14,183 | 5.21× |
| he | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,544 | 13,701 | 5.39× |
| he | `:currency currency=EUR currencySign=accounting` | 2,527 | 14,121 | 5.59× |
| he | `:currency currency=EUR fractionDigits=0` | 2,251 | 7,940 | 3.53× |
| he | `:unit unit=kilometer` | 3,732 | 12,780 | 3.42× |
| he | `:unit unit=kilometer unitDisplay=long` | 3,919 | 13,460 | 3.43× |
| he | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 4,867 | 16,311 | 3.35× |
| he | `:unit unit=celsius` | 3,698 | 12,697 | 3.43× |
| he | `:unit unit=kilogram unitDisplay=long` | 5,399 | 19,496 | 3.61× |
| he | `:unit unit=liter unitDisplay=long` | 4,548 | 17,718 | 3.90× |
| ja | `:currency currency=EUR` | 2,976 | 18,373 | 6.17× |
| ja | `:currency currency=USD` | 2,590 | 14,945 | 5.77× |
| ja | `:currency currency=JPY` | 3,735 | 19,966 | 5.35× |
| ja | `:currency currency=EUR currencyDisplay=code` | 3,135 | 17,458 | 5.57× |
| ja | `:currency currency=EUR currencyDisplay=name` | 3,014 | 17,334 | 5.75× |
| ja | `:currency currency=USD currencyDisplay=narrowSymbol` | 3,214 | 17,123 | 5.33× |
| ja | `:currency currency=EUR currencySign=accounting` | 3,664 | 21,950 | 5.99× |
| ja | `:currency currency=EUR fractionDigits=0` | 2,667 | 9,401 | 3.52× |
| ja | `:unit unit=kilometer` | 3,901 | 12,047 | 3.09× |
| ja | `:unit unit=kilometer unitDisplay=long` | 4,286 | 14,919 | 3.48× |
| ja | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 4,124 | 12,981 | 3.15× |
| ja | `:unit unit=celsius` | 3,547 | 13,628 | 3.84× |
| ja | `:unit unit=kilogram unitDisplay=long` | 3,769 | 13,007 | 3.45× |
| ja | `:unit unit=liter unitDisplay=long` | 3,372 | 12,021 | 3.56× |
| hi | `:currency currency=EUR` | 2,013 | 12,636 | 6.28× |
| hi | `:currency currency=USD` | 2,283 | 13,025 | 5.71× |
| hi | `:currency currency=JPY` | 2,119 | 11,881 | 5.61× |
| hi | `:currency currency=EUR currencyDisplay=code` | 2,719 | 14,762 | 5.43× |
| hi | `:currency currency=EUR currencyDisplay=name` | 2,589 | 14,449 | 5.58× |
| hi | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,777 | 14,478 | 5.21× |
| hi | `:currency currency=EUR currencySign=accounting` | 2,414 | 14,400 | 5.96× |
| hi | `:currency currency=EUR fractionDigits=0` | 2,066 | 7,348 | 3.56× |
| hi | `:unit unit=kilometer` | 3,589 | 11,854 | 3.30× |
| hi | `:unit unit=kilometer unitDisplay=long` | 3,801 | 12,634 | 3.32× |
| hi | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,906 | 12,642 | 3.24× |
| hi | `:unit unit=celsius` | 3,170 | 11,484 | 3.62× |
| hi | `:unit unit=kilogram unitDisplay=long` | 3,998 | 12,137 | 3.04× |
| hi | `:unit unit=liter unitDisplay=long` | 3,396 | 11,792 | 3.47× |
| ru | `:currency currency=EUR` | 2,043 | 13,083 | 6.40× |
| ru | `:currency currency=USD` | 2,600 | 13,192 | 5.07× |
| ru | `:currency currency=JPY` | 2,706 | 11,835 | 4.37× |
| ru | `:currency currency=EUR currencyDisplay=code` | 2,437 | 13,908 | 5.71× |
| ru | `:currency currency=EUR currencyDisplay=name` | 2,815 | 14,314 | 5.08× |
| ru | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,940 | 14,133 | 4.81× |
| ru | `:currency currency=EUR currencySign=accounting` | 2,551 | 14,402 | 5.65× |
| ru | `:currency currency=EUR fractionDigits=0` | 2,084 | 7,486 | 3.59× |
| ru | `:unit unit=kilometer` | 3,547 | 11,828 | 3.33× |
| ru | `:unit unit=kilometer unitDisplay=long` | 3,874 | 12,288 | 3.17× |
| ru | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,770 | 11,733 | 3.11× |
| ru | `:unit unit=celsius` | 3,323 | 10,541 | 3.17× |
| ru | `:unit unit=kilogram unitDisplay=long` | 3,701 | 11,294 | 3.05× |
| ru | `:unit unit=liter unitDisplay=long` | 3,449 | 11,496 | 3.33× |
| pl | `:currency currency=EUR` | 2,026 | 12,922 | 6.38× |
| pl | `:currency currency=USD` | 2,513 | 12,768 | 5.08× |
| pl | `:currency currency=JPY` | 2,275 | 10,934 | 4.81× |
| pl | `:currency currency=EUR currencyDisplay=code` | 2,305 | 12,709 | 5.51× |
| pl | `:currency currency=EUR currencyDisplay=name` | 2,564 | 12,381 | 4.83× |
| pl | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,742 | 13,023 | 4.75× |
| pl | `:currency currency=EUR currencySign=accounting` | 2,412 | 13,558 | 5.62× |
| pl | `:currency currency=EUR fractionDigits=0` | 1,991 | 7,121 | 3.58× |
| pl | `:unit unit=kilometer` | 3,097 | 9,453 | 3.05× |
| pl | `:unit unit=kilometer unitDisplay=long` | 3,422 | 10,501 | 3.07× |
| pl | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,873 | 10,272 | 2.65× |
| pl | `:unit unit=celsius` | 3,357 | 9,786 | 2.91× |
| pl | `:unit unit=kilogram unitDisplay=long` | 3,566 | 10,942 | 3.07× |
| pl | `:unit unit=liter unitDisplay=long` | 3,340 | 10,399 | 3.11× |
| cy | `:currency currency=EUR` | 2,035 | 12,819 | 6.30× |
| cy | `:currency currency=USD` | 2,131 | 11,852 | 5.56× |
| cy | `:currency currency=JPY` | 1,950 | 11,202 | 5.74× |
| cy | `:currency currency=EUR currencyDisplay=code` | 2,412 | 13,302 | 5.51× |
| cy | `:currency currency=EUR currencyDisplay=name` | 2,480 | 13,167 | 5.31× |
| cy | `:currency currency=USD currencyDisplay=narrowSymbol` | 2,422 | 12,149 | 5.02× |
| cy | `:currency currency=EUR currencySign=accounting` | 2,320 | 13,093 | 5.64× |
| cy | `:currency currency=EUR fractionDigits=0` | 1,918 | 6,796 | 3.54× |
| cy | `:unit unit=kilometer` | 2,930 | 10,136 | 3.46× |
| cy | `:unit unit=kilometer unitDisplay=long` | 3,312 | 11,245 | 3.40× |
| cy | `:unit unit=kilometer-per-hour unitDisplay=narrow` | 3,509 | 11,520 | 3.28× |
| cy | `:unit unit=celsius` | 3,145 | 12,029 | 3.82× |
| cy | `:unit unit=kilogram unitDisplay=long` | 3,832 | 11,661 | 3.04× |
| cy | `:unit unit=liter unitDisplay=long` | 3,167 | 11,152 | 3.52× |

</details>


## 4. Text against the Rust registry (`loc-rust.json`)

| Engine | family | `rt-names-cu` same / cases | `rust-cu` same / cases (self-check) |
|---|---|---:|---:|
| chromium 143.0.7499.4 | `:currency` | 1160 / 1232 | 1232 / 1232 |
| chromium 143.0.7499.4 | `:unit` | 857 / 924 | 924 / 924 |
| firefox 155.0 | `:currency` | 1202 / 1232 | 1232 / 1232 |
| firefox 155.0 | `:unit` | 899 / 924 | 924 / 924 |

### chromium 143.0.7499.4: where `rt-names-cu` differs

**`:currency`** — by locale: ar 14, he 16, cy 42.

| Class | cases | examples (locale, source: Rust → split) |
|---|---:|---|
| bidi-marks | 30 | ar `{|-1| :currency currency=EUR}`: `"‏‎-1.00 €"` → `"‏‎‎-1.00 €"`<br>ar `{|-1234.56| :currency currency=EUR}`: `"‏‎-1,234.56 €"` → `"‏‎‎-1,234.56 €"`<br>ar `{|-1| :currency currency=USD}`: `"‏‎-1.00 US$"` → `"‏‎‎-1.00 US$"` |
| symbols | 42 | cy `{|0| :currency currency=USD}`: `"US$0.00"` → `"$0.00"`<br>cy `{|1| :currency currency=USD}`: `"US$1.00"` → `"$1.00"`<br>cy `{|-1| :currency currency=USD}`: `"-US$1.00"` → `"-$1.00"` |

**`:unit`** — by locale: ar 13, he 12, cy 42.

| Class | cases | examples (locale, source: Rust → split) |
|---|---:|---|
| bidi-marks | 23 | ar `{|-1| :unit unit=kilometer}`: `"‎-1 كم"` → `"‎‎-1 كم"`<br>ar `{|-1234.56| :unit unit=kilometer}`: `"‎-1,234.56 كم"` → `"‎‎-1,234.56 كم"`<br>ar `{|-1| :unit unit=kilometer unitDisplay=long}`: `"‎-1 كيلومتر"` → `"‎‎-1 كيلومتر"` |
| digits | 2 | ar `{|1| :unit unit=liter unitDisplay=long}`: `"لتر"` → `"لتر1"`<br>ar `{|-1| :unit unit=liter unitDisplay=long}`: `"لتر"` → `"لتر1"` |
| symbols | 42 | cy `{|0| :unit unit=kilometer unitDisplay=long}`: `"0 km"` → `"0 kilometers"`<br>cy `{|1| :unit unit=kilometer unitDisplay=long}`: `"1 cilometr"` → `"1 kilometer"`<br>cy `{|-1| :unit unit=kilometer unitDisplay=long}`: `"-1 cilometr"` → `"-1 kilometer"` |

### firefox 155.0: where `rt-names-cu` differs

**`:currency`** — by locale: ar 14, he 16.

| Class | cases | examples (locale, source: Rust → split) |
|---|---:|---|
| bidi-marks | 30 | ar `{|-1| :currency currency=EUR}`: `"‏‎-1.00 €"` → `"‏‎‎-1.00 €"`<br>ar `{|-1234.56| :currency currency=EUR}`: `"‏‎-1,234.56 €"` → `"‏‎‎-1,234.56 €"`<br>ar `{|-1| :currency currency=USD}`: `"‏‎-1.00 US$"` → `"‏‎‎-1.00 US$"` |

**`:unit`** — by locale: ar 13, he 12.

| Class | cases | examples (locale, source: Rust → split) |
|---|---:|---|
| bidi-marks | 23 | ar `{|-1| :unit unit=kilometer}`: `"‎-1 كم"` → `"‎‎-1 كم"`<br>ar `{|-1234.56| :unit unit=kilometer}`: `"‎-1,234.56 كم"` → `"‎‎-1,234.56 كم"`<br>ar `{|-1| :unit unit=kilometer unitDisplay=long}`: `"‎-1 كيلومتر"` → `"‎‎-1 كيلومتر"` |
| digits | 2 | ar `{|1| :unit unit=liter unitDisplay=long}`: `"لتر"` → `"لتر1"`<br>ar `{|-1| :unit unit=liter unitDisplay=long}`: `"لتر"` → `"لتر1"` |

