# French: `*[0]` before `[one]` keeps 0 as a key of its own.
select-default-number = {$count ->
   *[0] Aucun fichier
    [one] Un fichier
    [other] {$count} fichiers
}
