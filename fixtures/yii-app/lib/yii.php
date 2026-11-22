<?php

/*
 * A stand-in for Yii 2, written for this fixture: the names of the classes and
 * methods the application uses, and nothing of Yii's code.
 */

namespace yii {
    class BaseYii
    {
        /** @var \yii\web\Application */
        public static $app;

        /** @var \yii\di\Container */
        public static $container;

        /** @return string */
        public static function t($category, $message, $params = [], $language = null)
        {
            return $message;
        }

        /** @return string|false */
        public static function getAlias($alias, $throwException = true)
        {
            return $alias;
        }

        public static function setAlias($alias, $path)
        {
        }

        /** @return object */
        public static function createObject($type, array $params = [])
        {
            return new \stdClass();
        }
    }
}

namespace yii\base {
    class BaseObject
    {
    }

    class Component extends BaseObject
    {
    }

    class Model extends Component
    {
        /** @return array */
        public function rules()
        {
            return [];
        }

        /** @return array */
        public function attributeLabels()
        {
            return [];
        }

        /** @return string */
        public function getAttributeLabel($attribute)
        {
            return $attribute;
        }

        public function addError($attribute, $error = '')
        {
        }

        /** @return bool */
        public function validate($attributeNames = null, $clearErrors = true)
        {
            return true;
        }

        /** @return bool */
        public function load($data, $formName = null)
        {
            return true;
        }
    }

    class Module extends \yii\di\ServiceLocator
    {
        /** @var string|bool|null */
        public $layout;

        /** @var string */
        public $controllerNamespace;

        /** @return Module|null */
        public function getModule($id, $load = true)
        {
            return null;
        }
    }

    class Application extends Module
    {
        /** @var array */
        public $params = [];
    }

    class Controller extends Component
    {
        /** @var string|bool|null */
        public $layout;

        /** @return string */
        public function render($view, $params = [])
        {
            return '';
        }

        /** @return string */
        public function renderPartial($view, $params = [])
        {
            return '';
        }
    }

    class View extends Component
    {
        /** @return string */
        public function render($view, $params = [], $context = null)
        {
            return '';
        }

        /** @return string */
        public function renderFile($viewFile, $params = [], $context = null)
        {
            return '';
        }
    }

    class Widget extends Component
    {
        /** @return string */
        public static function widget($config = [])
        {
            return '';
        }

        /** @return string */
        public function render($view, $params = [])
        {
            return '';
        }

        /** @return string|void */
        public function run()
        {
        }
    }
}

namespace yii\di {
    class ServiceLocator extends \yii\base\Component
    {
        /** @return object|null */
        public function get($id, $throwException = true)
        {
            return null;
        }

        /** @return bool */
        public function has($id, $checkInstance = false)
        {
            return false;
        }
    }

    class Container extends \yii\base\Component
    {
        /** @return object */
        public function get($class, $params = [], $config = [])
        {
            return new \stdClass();
        }
    }
}

namespace yii\web {
    /**
     * @property User $user
     * @property Request $request
     */
    class Application extends \yii\base\Application
    {
    }

    class Controller extends \yii\base\Controller
    {
        /** @return Response */
        public function redirect($url, $statusCode = 302)
        {
            return new Response();
        }
    }

    class View extends \yii\base\View
    {
        /** @var string */
        public $title;
    }

    class Request extends \yii\base\Component
    {
        /** @return array|mixed */
        public function post($name = null, $defaultValue = null)
        {
            return [];
        }
    }

    class Response extends \yii\base\Component
    {
    }

    /**
     * @property IdentityInterface|null $identity
     * @property bool $isGuest
     */
    class User extends \yii\base\Component
    {
    }

    interface IdentityInterface
    {
    }
}

namespace yii\db {
    class BaseActiveRecord extends \yii\base\Model
    {
        /** @return ActiveQuery */
        public static function find()
        {
            return new ActiveQuery();
        }

        /** @return static|null */
        public static function findOne($condition)
        {
            return null;
        }

        /** @return ActiveQuery */
        public function hasOne($class, $link)
        {
            return new ActiveQuery();
        }

        /** @return ActiveQuery */
        public function hasMany($class, $link)
        {
            return new ActiveQuery();
        }

        /** @return mixed */
        public function getAttribute($name)
        {
            return null;
        }
    }

    class ActiveRecord extends BaseActiveRecord
    {
        /** @return string */
        public static function tableName()
        {
            return '';
        }
    }

    class ColumnSchemaBuilder
    {
        /** @return $this */
        public function notNull()
        {
            return $this;
        }

        /** @return $this */
        public function defaultValue($default)
        {
            return $this;
        }
    }

    class Migration extends \yii\base\Component
    {
        public function createTable($table, $columns, $options = null)
        {
        }

        public function dropTable($table)
        {
        }

        public function addColumn($table, $column, $type)
        {
        }

        /** @return ColumnSchemaBuilder */
        public function primaryKey($length = null)
        {
            return new ColumnSchemaBuilder();
        }

        /** @return ColumnSchemaBuilder */
        public function string($length = null)
        {
            return new ColumnSchemaBuilder();
        }

        /** @return ColumnSchemaBuilder */
        public function integer($length = null)
        {
            return new ColumnSchemaBuilder();
        }
    }

    class ActiveQuery
    {
        /** @return $this */
        public function where($condition)
        {
            return $this;
        }

        /** @return $this */
        public function orderBy($columns)
        {
            return $this;
        }

        /** @return array|ActiveRecord|null */
        public function one($db = null)
        {
            return null;
        }

        /** @return array */
        public function all($db = null)
        {
            return [];
        }
    }

    class Connection extends \yii\base\Component
    {
        /** @var string */
        public $dsn;
    }
}

namespace yii\helpers {
    class BaseUrl
    {
        /** @return string */
        public static function to($url = '', $scheme = false)
        {
            return '';
        }

        /** @return string */
        public static function toRoute($route, $scheme = false)
        {
            return '';
        }
    }

    class Url extends BaseUrl
    {
    }

    class BaseHtml
    {
        /** @return string */
        public static function a($text, $url = null, $options = [])
        {
            return '';
        }

        /** @return string */
        public static function encode($content, $doubleEncode = true)
        {
            return (string) $content;
        }
    }

    class Html extends BaseHtml
    {
    }
}

namespace yii\caching {
    class FileCache extends \yii\base\Component
    {
        /** @return bool */
        public function flush()
        {
            return true;
        }
    }
}

namespace yii\mail {
    interface MessageInterface
    {
        /** @return $this */
        public function setTo($to);

        /** @return bool */
        public function send();
    }

    interface MailerInterface
    {
        /** @return MessageInterface */
        public function compose($view = null, array $params = []);
    }

    abstract class BaseMailer extends \yii\base\Component implements MailerInterface
    {
    }
}

namespace {
    defined('YII_ENV') or define('YII_ENV', 'prod');
    defined('YII_ENV_DEV') or define('YII_ENV_DEV', YII_ENV === 'dev');

    class Yii extends \yii\BaseYii
    {
    }
}
