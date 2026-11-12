"""Run only against checked-in synthetic fixture copies, never a user's database."""
from pathlib import Path
import shutil, sqlite3, subprocess, sys
fixture = Path(sys.argv[1]); native = Path(sys.argv[2]); work = Path(sys.argv[3])
cases = {
 'extra-column': 'ALTER TABLE DeletedDialog ADD COLUMN unused TEXT',
 'missing-nullable': 'ALTER TABLE DeletedDialog DROP COLUMN folderId',
 'missing-required': 'ALTER TABLE DeletedMessage DROP COLUMN views',
 'v0': 'UPDATE SchemaVersion SET version=0',
 'missing-version-row': 'DELETE FROM SchemaVersion;INSERT INTO SchemaVersion VALUES(7,77)',
 'overflow-version': 'UPDATE SchemaVersion SET version=4294967373',
 'future-version': 'UPDATE SchemaVersion SET version=77',
 'changed-nullability': 'PRAGMA writable_schema=ON;UPDATE sqlite_master SET sql=replace(sql,\'"folderId" INTEGER NULL\',\'"folderId" INTEGER NOT NULL\') WHERE name=\'DeletedDialog\';PRAGMA writable_schema=OFF;UPDATE DeletedDialog SET folderId=0 WHERE folderId IS NULL',
 'changed-type-ignored': 'PRAGMA writable_schema=ON;UPDATE sqlite_master SET sql=replace(sql,\'"views" INTEGER NOT NULL\',\'"views" TEXT NOT NULL\') WHERE name=\'DeletedMessage\';PRAGMA writable_schema=OFF',
}
for name,sql in cases.items():
 root=work/name; (root/'tdata').mkdir(parents=True,exist_ok=True)
 path=root/'tdata/ayudata.db';shutil.copyfile(fixture/'original-ayudata.db',path)
 with sqlite3.connect(path) as db: db.executescript(sql)
 shutil.copyfile(path,fixture/(name+'.input.db'))
 p=subprocess.run([str(native),'read',str(root)],text=True,errors="backslashreplace",capture_output=True,check=True)
 shutil.copyfile(path,fixture/(name+'.original.db'))
 (fixture/(name+'.native.txt')).write_text(p.stdout.splitlines()[0]+"\n"+p.stderr)
