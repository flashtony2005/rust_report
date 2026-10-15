import json,sys
d=json.load(open(sys.argv[1]))
for x in d['browsers']:
    if 'chromium' in x['name']: print(' ',x['name'],x['revision'])
