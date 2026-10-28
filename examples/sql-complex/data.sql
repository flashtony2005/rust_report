CREATE TABLE dim_region(region TEXT PRIMARY KEY, manager TEXT);
INSERT INTO dim_region VALUES
 ('华东','林岚'),('华北','李禾'),('华南','许远'),('西南','赵青');

CREATE TABLE dim_product(product_id INTEGER PRIMARY KEY, product TEXT, category TEXT);
INSERT INTO dim_product VALUES
 (10,'标准版','软件'),(11,'专业版','软件'),(20,'现场服务','服务'),(21,'培训','服务'),(30,'硬件盒子','硬件');

CREATE TABLE dim_customer(customer_id INTEGER PRIMARY KEY, customer TEXT, segment TEXT);
INSERT INTO dim_customer VALUES
 (1,'Acme','企业'),(2,'Birch','企业'),(3,'Cedar','中小'),(4,'Dune','中小'),(5,'Elm','企业');

CREATE TABLE fact_sales(
 id INTEGER PRIMARY KEY, order_date TEXT, region TEXT, product_id INTEGER,
 customer_id INTEGER, qty INTEGER, price REAL, cost REAL, status TEXT);
INSERT INTO fact_sales VALUES
 (1,'2024-01-05','华东',10,1,10,500,220,'paid'),
 (2,'2024-01-06','华东',11,2,3,900,420,'paid'),
 (3,'2024-01-10','华北',20,3,5,700,350,'paid'),
 (4,'2024-01-11','华北',21,4,2,300,120,'paid'),
 (5,'2024-01-15','华南',30,5,4,1200,780,'paid'),
 (6,'2024-01-18','西南',10,1,6,500,220,'paid'),
 (7,'2024-01-20','华东',20,2,3,700,350,'paid'),
 (8,'2024-01-22','华北',10,3,8,500,220,'cancelled'),
 (9,'2024-02-02','华东',11,1,6,900,420,'paid'),
 (10,'2024-02-03','华北',20,2,7,700,350,'paid'),
 (11,'2024-02-05','华南',30,3,2,1200,780,'paid'),
 (12,'2024-02-08','西南',10,4,4,500,220,'paid'),
 (13,'2024-02-10','华东',10,5,5,500,220,'paid'),
 (14,'2024-02-12','华北',11,1,2,900,420,'paid'),
 (15,'2024-02-15','华南',20,2,4,700,350,'paid'),
 (16,'2024-03-01','华东',30,3,3,1200,780,'paid'),
 (17,'2024-03-03','华北',10,4,9,500,220,'paid'),
 (18,'2024-03-05','华南',11,5,4,900,420,'paid'),
 (19,'2024-03-07','西南',20,2,6,700,350,'paid'),
 (20,'2024-03-09','华东',21,1,5,300,120,'paid'),
 (21,'2024-03-12','华北',30,3,2,1200,780,'paid'),
 (22,'2024-03-15','华南',10,4,3,500,220,'paid');

CREATE TABLE fact_returns(
 id INTEGER PRIMARY KEY, sale_id INTEGER, return_date TEXT, amount REAL);
INSERT INTO fact_returns VALUES
 (1,3,'2024-01-25',70.0),
 (2,10,'2024-02-20',140.0),
 (3,15,'2024-03-02',80.0),
 (4,19,'2024-03-20',120.0);
