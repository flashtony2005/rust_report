CREATE TABLE ledger(region TEXT, revenue INTEGER, cost INTEGER);
INSERT INTO ledger VALUES ('华东',1200,700),('华东',800,400),('华北',900,500),('西南',600,350);

CREATE TABLE slice_orders(order_id INTEGER, ordered_at TEXT, region TEXT, city TEXT, employee_id INTEGER, customer TEXT, shipper_id INTEGER, shipping INTEGER);
INSERT INTO slice_orders VALUES
 (101,'2024-01-15','East','Shared',1,'Acme',1,7),
 (102,'2024-01-20','West','Shared',2,'Birch',2,11),
 (103,'2024-02-29','East','E2',1,'Cedar',1,13),
 (104,'2024-02-01','West','W2',3,'Dune',2,17),
 (105,'2025-01-02','East','Shared',2,'Elm',1,19),
 (106,'2025-02-03','West','Shared',3,'Fern',2,23),
 (107,'2025-01-03','West','W2',1,'Grove',1,29),
 (108,'2025-03-01','West','W3',2,'Hazel',2,31),
 (109,'2025-03-02','East','E2',4,'Iris',1,37);
CREATE TABLE slice_lines(line_id INTEGER, order_id INTEGER, product_id INTEGER, qty INTEGER, price INTEGER);
-- 故意打乱年月、地区的顺序；同一订单 101 有两条明细，109 无明细。
INSERT INTO slice_lines VALUES
 (1,108,21,8,100),(2,103,10,3,100),(3,101,10,2,50),
 (4,107,10,7,100),(5,102,20,2,100),(6,106,20,6,100),
 (7,101,11,1,50),(8,104,21,4,100),(9,105,11,5,100);
