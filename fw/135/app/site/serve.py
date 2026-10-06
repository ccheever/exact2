import http.server, functools, sys, socket
class H(http.server.SimpleHTTPRequestHandler):
    def log_message(self, fmt, *a):
        sys.stderr.write("%s Host=%s Referer=%s\n" % (self.requestline, self.headers.get("Host"), self.headers.get("Referer"))); sys.stderr.flush()
    def end_headers(self):
        self.send_header("Cache-Control", "no-store"); super().end_headers()
class S(http.server.ThreadingHTTPServer):
    address_family = socket.AF_INET6
    def server_bind(self):
        self.socket.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 0); super().server_bind()
S(("::", int(sys.argv[2])), functools.partial(H, directory=sys.argv[1])).serve_forever()
