select-plural = {$count ->
    [one] One file
   *[other] {$count} files
}
select-string = {$gender ->
    [masculine] his
    [feminine] her
   *[other] their
}
select-number-key = {$count ->
    [0] No files
    [one] One file
   *[other] {$count} files
}
select-default-number = {$count ->
    [one] One file
   *[0] No files
}
select-number-function = {NUMBER($score, minimumFractionDigits: 1) ->
    [1.0] Exactly one
    [one] One-ish
   *[other] {$score}
}
select-ordinal = {NUMBER($place, type: "ordinal") ->
    [one] {$place}st
    [two] {$place}nd
    [few] {$place}rd
   *[other] {$place}th
}
select-two-annotations = {$n ->
    [one] one
   *[other] {NUMBER($n, type: "ordinal") ->
        [one] first-ish
       *[other] other-ish
    }
}
select-nested = {$gender ->
    [feminine] She has {$count ->
        [one] a cat
       *[other] cats
    }
   *[other] They have {$count ->
        [one] a cat
       *[other] cats
    }
}
select-siblings = {$a ->
    [x] X
   *[y] Y
} and {$b ->
    [one] one
   *[other] many
}
select-in-text = Before {$kind ->
    [a] A
   *[b] B
} after.
select-only-other = {$count ->
   *[other] Always this
}
select-string-literal = {"b" ->
    [a] not this
    [b] this
   *[c] nor this
}
select-number-literal = {1 ->
    [one] singular
   *[other] plural
}
select-term-attribute = The {-term-plain.kind ->
    [masculine] he
    [neuter] it
   *[other] they
} is here.
select-term-variable = {-term-select(count: 1)}
-term-select = {$count ->
    [one] a single thing
   *[other] things
}
select-number-literal-function = {NUMBER(2.0, minimumFractionDigits: 1) ->
    [2.0] two point oh
   *[other] something else
}
