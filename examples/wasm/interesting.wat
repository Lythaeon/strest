(module
  (memory (export "memory") 1 2)
  (data (i32.const 0) "{\"schema_version\":1,\"base_url\":\"http://localhost:8887\",\"vars\":{\"user\":\"demo\",\"pass\":\"secret\",\"query\":\"perf\",\"item\":\"42\"},\"steps\":[{\"name\":\"health\",\"method\":\"get\",\"path\":\"/health\",\"assert_status\":200},{\"name\":\"login\",\"method\":\"post\",\"path\":\"/login\",\"headers\":[\"Content-Type: application/json\"],\"data\":\"{\\\"user\\\":\\\"{{user}}\\\",\\\"pass\\\":\\\"{{pass}}\\\"}\",\"assert_status\":200},{\"name\":\"search\",\"method\":\"get\",\"path\":\"/search?q={{query}}\",\"assert_status\":200,\"think_time\":\"150ms\"},{\"name\":\"details\",\"method\":\"get\",\"path\":\"/items/{{item}}\",\"assert_status\":200,\"think_time\":\"100ms\"},{\"name\":\"checkout\",\"method\":\"post\",\"path\":\"/checkout\",\"headers\":[\"Content-Type: application/json\"],\"data\":\"{\\\"item\\\":\\\"{{item}}\\\",\\\"qty\\\":1}\",\"assert_status\":200}]}")
  (func (export "scenario_ptr") (result i32) (i32.const 0))
  (func (export "scenario_len") (result i32) (i32.const 736))
)
