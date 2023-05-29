select crate_owners.id
from crates_index.tokens
         join crates_index.crate_owners on tokens.user_id = crate_owners.user_id
         join crates_index.crates on crate_owners.crate_id = crates.id
where token_hash = crates_index.crypt($1, tokens.token_hash)
  and crates.name = $2
  and crates.registry is null;