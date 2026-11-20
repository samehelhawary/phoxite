// Written for the fixture: the names PHP declares, as a script writes them.
registerBlockType('shop/stock', {});

jQuery.post(ajaxurl, { action: 'shop_add', id: 1 });

apiFetch({ path: '/shop/v1/books' });
