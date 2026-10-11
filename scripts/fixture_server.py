"""Disposable local API for Force QA. Contains synthetic data only."""
import json
import uuid
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs


def now():
    return datetime.now(timezone.utc)


def iso(value):
    return value.isoformat().replace('+00:00', 'Z')


tags = [dict(id=str(i), name=name, kind=kind, sortOrder=i, archivedAt=None) for i, (name, kind) in enumerate([
    ('工作', 'positive'), ('运动', 'positive'), ('阅读', 'neutral'), ('刷视频', 'negative'),
    ('休息', 'leisure'), ('操作系统', 'positive'), ('软件工程', 'positive'),
    ('信息与网络安全：这是一个用来检查截断和边界的长标签名称', 'positive'),
])]
sessions = []
logs = [dict(id='log-1', tag='工作', startTime=iso(now()-timedelta(hours=2)), endTime=iso(now()-timedelta(hours=1)), duration=3600)]


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def respond(self, value, status=200):
        data = json.dumps(value, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        path = urlparse(self.path).path
        if path == '/focus/tags': self.respond(tags)
        elif path == '/focus/current': self.respond(sessions)
        elif path == '/focus/logs':
            query = parse_qs(urlparse(self.path).query)
            start = query.get('startDate', ['0000'])[0]
            end = query.get('endDate', ['9999'])[0]
            self.respond([log for log in logs if start <= log['startTime'] <= end])
        elif path == '/focus/week-stats':
            monday = now().date()-timedelta(days=now().weekday())
            self.respond(dict(weekStart=str(monday), weekEnd=str(monday+timedelta(days=6)), totalDuration=sum(l['duration'] for l in logs), totalCount=len(logs), dailyStats=[dict(date=str(monday+timedelta(days=i)), totalDuration=3600 if i==now().weekday() else 0, count=1 if i==now().weekday() else 0) for i in range(7)], tagStats=[dict(tag='工作', duration=3600, count=1)]))
        else: self.respond(dict(message='Not found'), 404)

    def do_POST(self):
        global sessions
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', 0))))
        path = urlparse(self.path).path
        if path == '/focus/start':
            for session in sessions:
                if session['tag'] == body['tag']: return self.respond(session)
            session = dict(id=str(uuid.uuid4()), tag=body['tag'], startTime=iso(now()))
            sessions.append(session)
            self.respond(session, 201)
        elif path == '/focus/stop':
            selected = [s for s in sessions if 'tag' not in body or s['tag'] == body['tag']]
            end = now()
            new_logs = [dict(id=str(uuid.uuid4()), tag=s['tag'], startTime=s['startTime'], endTime=iso(end), duration=int((end-datetime.fromisoformat(s['startTime'].replace('Z', '+00:00'))).total_seconds())) for s in selected]
            logs.extend(new_logs)
            sessions = [s for s in sessions if s not in selected]
            self.respond(new_logs)
        elif path == '/focus/tags':
            tag = dict(id=str(uuid.uuid4()), name=body['name'], kind=body['kind'], sortOrder=len(tags), archivedAt=None)
            tags.append(tag)
            self.respond(tag, 201)
        else: self.respond(dict(message='Not found'), 404)


if __name__ == '__main__':
    print('Synthetic Force API on http://127.0.0.1:3459', flush=True)
    ThreadingHTTPServer(('127.0.0.1', 3459), Handler).serve_forever()
