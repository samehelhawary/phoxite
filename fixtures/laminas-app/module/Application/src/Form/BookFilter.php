<?php

namespace Application\Form;

class BookFilter
{
    public function getInputFilterSpecification(): array
    {
        return [
            'title' => [
                'required' => true,
                'validators' => [
                    ['name' => 'StringLength', 'options' => ['max' => 200]],
                ],
            ],
        ];
    }
}
