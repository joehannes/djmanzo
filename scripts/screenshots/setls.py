"""setls.py DB key=value ... (value '' deletes) -- write WebKit localStorage."""
import sqlite3,sys
c=sqlite3.connect(sys.argv[1])
for kv in sys.argv[2:]:
    k,v=kv.split("=",1)
    if v=="": c.execute("delete from ItemTable where key=?",(k,))
    else: c.execute("insert into ItemTable(key,value) values(?,?)",(k,v.encode("utf-16-le")))
c.commit()
