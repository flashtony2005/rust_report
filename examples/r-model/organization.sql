CREATE TABLE organization(region TEXT, owner TEXT, display_order INTEGER);
INSERT INTO organization VALUES ('华东','林岚',1),('华北','李禾',2),('西南','赵青',3),('华南','许远',4);

CREATE TABLE slice_carriers(shipper_id INTEGER, company TEXT);
INSERT INTO slice_carriers VALUES (1,'北辰物流'),(2,'云帆运输');
