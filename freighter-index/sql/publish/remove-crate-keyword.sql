delete
from crate_keywords
    using keywords
where crate = $1
  and keyword = keywords.id
  and keywords.name = $2
