"""Offline reconstructed complete-result transfer/parse controls; no live RPC."""
import argparse, hashlib, json, pathlib, socket, statistics, struct, threading, time

def exact(stream, count):
    result = bytearray()
    chunks = 0
    while len(result) < count:
        chunk = stream.recv(count-len(result))
        if not chunk:
            raise ValueError('truncated profile response')
        result.extend(chunk)
        chunks += 1
    return bytes(result), chunks

def quantiles(rows, name):
    values = sorted(row[name] for row in rows)
    at = .95*(len(values)-1)
    lo = int(at)
    return {'median':statistics.median(values),'p95':values[lo]+(values[min(lo+1,len(values)-1)]-values[lo])*(at-lo),'max':max(values)}

def group(frame, expected, parse, requested):
    gate = threading.Barrier(8, timeout=5)
    clients, servers, replies, failures, threads, sockets = [], [], [], [], [], []
    lock = threading.Lock()
    def failure(error):
        with lock: failures.append(repr(error))
        gate.abort()
    for ordinal in range(4):
        sender, receiver = socket.socketpair()
        sockets.extend((sender,receiver))
        sender.settimeout(5); receiver.settimeout(5)
        if requested: sender.setsockopt(socket.SOL_SOCKET,socket.SO_SNDBUF,requested)
        actual = sender.getsockopt(socket.SOL_SOCKET,socket.SO_SNDBUF)
        def send(sender=sender, ordinal=ordinal, actual=actual):
            try:
                gate.wait()
                wall=time.monotonic_ns(); cpu=time.thread_time_ns()
                sender.sendall(frame)
                row={'ordinal':ordinal,'send_wall_ms':(time.monotonic_ns()-wall)/1e6,
                     'send_thread_cpu_ms':(time.thread_time_ns()-cpu)/1e6,'send_buffer_actual_bytes':actual}
                with lock: servers.append(row)
            except BaseException as error: failure(error)
            finally: sender.close()
        def receive(receiver=receiver,ordinal=ordinal):
            try:
                gate.wait()
                began=time.monotonic_ns(); cpu=time.thread_time_ns()
                header,_=exact(receiver,4)
                count=struct.unpack('>I',header)[0]
                assert count==len(frame)-4
                payload,chunks=exact(receiver,count)
                read_end=time.monotonic_ns(); read_cpu=time.thread_time_ns()
                if parse: decoded=json.loads(payload)
                parse_end=time.monotonic_ns(); parse_cpu=time.thread_time_ns()
                with lock:
                    clients.append({'ordinal':ordinal,'whole_wall_ms':(parse_end-began)/1e6,
                                    'transfer_wall_ms':(read_end-began)/1e6,
                                    'transfer_thread_cpu_ms':(read_cpu-cpu)/1e6,
                                    'parse_wall_ms':(parse_end-read_end)/1e6,
                                    'parse_thread_cpu_ms':(parse_cpu-read_cpu)/1e6,'recv_chunks':chunks})
                    if not parse: replies.append(payload)
                assert header+payload==frame
                if parse: assert decoded==expected
            except BaseException as error: failure(error)
            finally: receiver.close()
        threads.extend((threading.Thread(target=send),threading.Thread(target=receive)))
    try:
        for thread in threads: thread.start()
        deadline=time.monotonic()+15
        for thread in threads: thread.join(max(0,deadline-time.monotonic()))
        if any(thread.is_alive() for thread in threads):
            for stream in sockets:
                try: stream.shutdown(socket.SHUT_RDWR)
                except OSError: pass
            for thread in threads: thread.join(6)
            raise TimeoutError('owned socket-profile threads exceeded finite group lifetime')
        assert not failures, failures
        assert len(clients)==len(servers)==4
        for payload in replies: assert json.loads(payload)==expected
        return clients,servers
    finally:
        for stream in sockets: stream.close()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--frame',type=pathlib.Path,required=True)
    parser.add_argument('--out',type=pathlib.Path,required=True)
    args=parser.parse_args()
    args.out.mkdir(mode=0o700,exist_ok=False)
    with args.frame.open('rb') as stream: frame=stream.read(8*1024*1024+1)
    assert 4 < len(frame) <= 8*1024*1024
    assert struct.unpack('>I',frame[:4])[0]==len(frame)-4
    expected=json.loads(frame[4:])
    assert expected['version']==1 and expected['body']['response']=='result'
    document=expected['body']['value']
    assert document['job']['currentness']=='current' and document['result']['currentness']=='current'
    assert document['repair']['batch']['full']==document['result'] and document['repair']['accepted']
    record={'kind':'offline_reconstructed_complete_result_transfer_parse','live_acceptance':False,
            'frame_sha256':hashlib.sha256(frame).hexdigest(),'frame_bytes':len(frame),
            'graph_complete':True,'producer':'four owned connected AF_UNIX socketpairs; no actual daemon/freshness',
            'client_receive':'original exact bytearray recv/extend/bytes algorithm; full json.loads; validation outside timer',
            'raw_control':'all full bodies parsed and graph-validated after each four-reader transfer group',
            'socket_timeout_s':5,'group_lifetime_s':15,'results':[]}
    # Fixed alternating conditions avoid a best-run selection; all rows retained.
    for group_index in range(8):
        for parse,requested in ((True,0),(True,4*1024*1024),(False,0),(False,4*1024*1024)):
            clients,servers=group(frame,expected,parse,requested)
            record['results'].append({'group':group_index,'parse_inside_clock':parse,
                                      'requested_send_buffer_bytes':requested,'clients':clients,'servers':servers})
            (args.out/'raw.json').write_text(json.dumps(record,indent=2)+'\n')
    summary=[]
    for parse,requested in ((True,0),(True,4*1024*1024),(False,0),(False,4*1024*1024)):
        groups=[row for row in record['results'] if row['parse_inside_clock']==parse and row['requested_send_buffer_bytes']==requested]
        clients=[client for row in groups for client in row['clients']]
        servers=[server for row in groups for server in row['servers']]
        summary.append({'parse_inside_clock':parse,'requested_send_buffer_bytes':requested,'samples':len(clients),
                        'clients':{key:quantiles(clients,key) for key in ('whole_wall_ms','transfer_wall_ms','transfer_thread_cpu_ms','parse_wall_ms','parse_thread_cpu_ms','recv_chunks')},
                        'servers':{key:quantiles(servers,key) for key in ('send_wall_ms','send_thread_cpu_ms','send_buffer_actual_bytes')}})
    (args.out/'summary.json').write_text(json.dumps(dict(record,results=summary),indent=2)+'\n')

if __name__=='__main__': main()
