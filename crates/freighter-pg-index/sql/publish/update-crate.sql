update crates
set description   = $2,
    documentation = $3,
    homepage      = $4,
    repository    = $5
where id = $1
