delete
from freighter_crate_owners using freighter_users
where username = $1
  and crate = $2
  and freighter_crate_owners.user_id = freighter_users.id