Here's how the shop's data fits together. Orders sit in the middle: a customer places them, each line points at a product variant, and payments, shipments and refunds hang off the order.

```dbml
Table customers {
  id uuid [pk]
  email varchar [unique, not null]
  name varchar
}

Table products {
  id uuid [pk]
  title varchar [not null]
  published bool [default: false]
}

Table variants {
  id uuid [pk]
  product_id uuid [not null]
  sku varchar [unique]
  price_cents int [not null]
}

Table orders [headercolor: #e0864a] {
  id uuid [pk]
  customer_id uuid [not null]
  status varchar [default: 'cart']
  total_cents int
}

Table order_items {
  order_id uuid [not null]
  variant_id uuid [not null]
  quantity int [not null]
}

Table payments {
  id uuid [pk]
  order_id uuid [not null]
  amount_cents int [not null]
}

Ref: orders.customer_id > customers.id
Ref: variants.product_id > products.id
Ref: order_items.order_id > orders.id
Ref: order_items.variant_id > variants.id
Ref: payments.order_id > orders.id
```

`order_items.price_cents` is worth adding so an order keeps the price it was placed at when a variant's price changes later.
